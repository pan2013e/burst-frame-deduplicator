#![cfg(all(target_os = "linux", feature = "libraw-preview"))]

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_ushort, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr::NonNull;

use anyhow::{Context, anyhow, bail};
use libloading::Library;

const LIBRAW_IMAGE_JPEG: c_int = 1;
const LIBRAW_IMAGE_BITMAP: c_int = 2;
const MAX_THUMBNAIL_BYTES: usize = 256 * 1024 * 1024;
const LIBRARY_CANDIDATES: &[&str] = &["libraw_r.so.23", "libraw_r.so", "libraw.so.23", "libraw.so"];

type InitFn = unsafe extern "C" fn(c_uint) -> *mut c_void;
type OpenFileFn = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type UnpackThumbFn = unsafe extern "C" fn(*mut c_void) -> c_int;
type MakeMemThumbFn = unsafe extern "C" fn(*mut c_void, *mut c_int) -> *mut LibRawProcessedImage;
type ClearMemFn = unsafe extern "C" fn(*mut LibRawProcessedImage);
type CloseFn = unsafe extern "C" fn(*mut c_void);
type StrErrorFn = unsafe extern "C" fn(c_int) -> *const c_char;

#[repr(C)]
struct LibRawProcessedImage {
    image_type: c_int,
    height: c_ushort,
    width: c_ushort,
    colors: c_ushort,
    bits: c_ushort,
    data_size: c_uint,
    data: [u8; 1],
}

struct RawHandle {
    pointer: NonNull<c_void>,
    close: CloseFn,
}

impl Drop for RawHandle {
    fn drop(&mut self) {
        // SAFETY: `pointer` came from `libraw_init`, remains owned by this guard, and is closed once.
        unsafe { (self.close)(self.pointer.as_ptr()) };
    }
}

struct MemoryImage {
    pointer: NonNull<LibRawProcessedImage>,
    clear: ClearMemFn,
}

impl Drop for MemoryImage {
    fn drop(&mut self) {
        // SAFETY: `pointer` came from `libraw_dcraw_make_mem_thumb` and is cleared exactly once.
        unsafe { (self.clear)(self.pointer.as_ptr()) };
    }
}

pub fn extract_embedded_preview(path: &Path) -> anyhow::Result<Vec<u8>> {
    let library = load_library()?;
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let init: InitFn = unsafe { *library.get(b"libraw_init\0")? };
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let open_file: OpenFileFn = unsafe { *library.get(b"libraw_open_file\0")? };
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let unpack_thumb: UnpackThumbFn = unsafe { *library.get(b"libraw_unpack_thumb\0")? };
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let make_mem_thumb: MakeMemThumbFn = unsafe { *library.get(b"libraw_dcraw_make_mem_thumb\0")? };
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let clear_mem: ClearMemFn = unsafe { *library.get(b"libraw_dcraw_clear_mem\0")? };
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let close: CloseFn = unsafe { *library.get(b"libraw_close\0")? };
    // SAFETY: Symbols and signatures match LibRaw's public C ABI in libraw.h.
    let strerror: StrErrorFn = unsafe { *library.get(b"libraw_strerror\0")? };

    // SAFETY: A zero flag value is supported by LibRaw and requires no caller-owned state.
    let pointer = unsafe { init(0) };
    let handle = RawHandle {
        pointer: NonNull::new(pointer).ok_or_else(|| anyhow!("LibRaw initialization failed"))?,
        close,
    };
    let path = CString::new(path.as_os_str().as_bytes())
        .context("RAW path contains an embedded NUL byte")?;

    // SAFETY: The handle is live and `path` is a NUL-terminated string for this call.
    check_result(
        unsafe { open_file(handle.pointer.as_ptr(), path.as_ptr()) },
        strerror,
    )
    .context("opening RAW file with LibRaw")?;
    // SAFETY: The handle owns a successfully opened RAW file.
    check_result(unsafe { unpack_thumb(handle.pointer.as_ptr()) }, strerror)
        .context("unpacking the embedded RAW preview")?;

    let mut error = 0;
    // SAFETY: The handle remains live until after the returned memory image is cleared.
    let memory = unsafe { make_mem_thumb(handle.pointer.as_ptr(), &mut error) };
    if error != 0 {
        return Err(libraw_error(error, strerror)).context("creating the embedded RAW preview");
    }
    let memory = MemoryImage {
        pointer: NonNull::new(memory)
            .ok_or_else(|| anyhow!("LibRaw returned an empty embedded preview"))?,
        clear: clear_mem,
    };
    // SAFETY: The memory-image guard owns a valid LibRaw allocation for this scope.
    let image = unsafe { memory.pointer.as_ref() };
    let data_size = usize::try_from(image.data_size).context("invalid LibRaw preview size")?;
    if data_size == 0 || data_size > MAX_THUMBNAIL_BYTES {
        bail!("LibRaw embedded preview has an invalid byte size: {data_size}");
    }
    // SAFETY: LibRaw declares `data_size` bytes beginning at the flexible `data` member.
    let data = unsafe { std::slice::from_raw_parts(image.data.as_ptr(), data_size) };
    match image.image_type {
        LIBRAW_IMAGE_JPEG => {
            if !data.starts_with(&[0xff, 0xd8]) {
                bail!("LibRaw embedded JPEG has an invalid header");
            }
            Ok(data.to_vec())
        }
        LIBRAW_IMAGE_BITMAP => bitmap_to_pnm(
            data,
            usize::from(image.width),
            usize::from(image.height),
            usize::from(image.colors),
            usize::from(image.bits),
        ),
        image_type => bail!("LibRaw returned unsupported preview type {image_type}"),
    }
}

fn load_library() -> anyhow::Result<Library> {
    let mut errors = Vec::new();
    for candidate in LIBRARY_CANDIDATES {
        // SAFETY: Loading a named library is contained here; all accessed symbols use the C ABI.
        match unsafe { Library::new(*candidate) } {
            Ok(library) => return Ok(library),
            Err(error) => errors.push(format!("{candidate}: {error}")),
        }
    }
    bail!(
        "LibRaw runtime library is unavailable: {}",
        errors.join("; ")
    )
}

fn check_result(code: c_int, strerror: StrErrorFn) -> anyhow::Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(libraw_error(code, strerror))
    }
}

fn libraw_error(code: c_int, strerror: StrErrorFn) -> anyhow::Error {
    // SAFETY: LibRaw returns either a static NUL-terminated message or null for an unknown code.
    let message = unsafe {
        let pointer = strerror(code);
        (!pointer.is_null()).then(|| CStr::from_ptr(pointer).to_string_lossy().into_owned())
    };
    anyhow!(
        "LibRaw error {code}: {}",
        message.unwrap_or_else(|| "unknown error".to_string())
    )
}

fn bitmap_to_pnm(
    data: &[u8],
    width: usize,
    height: usize,
    colors: usize,
    bits: usize,
) -> anyhow::Result<Vec<u8>> {
    if width == 0 || height == 0 {
        bail!("LibRaw bitmap preview has invalid dimensions {width}x{height}");
    }
    if !matches!(colors, 1 | 3 | 4) || !matches!(bits, 8 | 16) {
        bail!("LibRaw bitmap preview uses unsupported {colors}-channel {bits}-bit pixels");
    }
    let bytes_per_sample = bits / 8;
    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| anyhow!("LibRaw bitmap dimensions overflow"))?;
    let required = pixels
        .checked_mul(colors)
        .and_then(|value| value.checked_mul(bytes_per_sample))
        .ok_or_else(|| anyhow!("LibRaw bitmap byte count overflows"))?;
    if data.len() < required {
        bail!(
            "LibRaw bitmap is truncated: expected {required} bytes, received {}",
            data.len()
        );
    }

    let output_colors = if colors == 1 { 1 } else { 3 };
    let payload_size = pixels * output_colors * bytes_per_sample;
    let magic = if output_colors == 1 { "P5" } else { "P6" };
    let maximum = if bits == 8 { 255 } else { 65_535 };
    let header = format!("{magic}\n{width} {height}\n{maximum}\n");
    let mut output = Vec::with_capacity(header.len() + payload_size);
    output.extend_from_slice(header.as_bytes());

    for pixel in data[..required].chunks_exact(colors * bytes_per_sample) {
        for channel in 0..output_colors {
            let offset = channel * bytes_per_sample;
            if bits == 8 {
                output.push(pixel[offset]);
            } else {
                let value = u16::from_ne_bytes([pixel[offset], pixel[offset + 1]]);
                output.extend_from_slice(&value.to_be_bytes());
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Instant;

    use super::{LibRawProcessedImage, bitmap_to_pnm, extract_embedded_preview};

    #[test]
    fn processed_image_layout_matches_libraw_v23() {
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, image_type), 0);
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, height), 4);
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, width), 6);
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, colors), 8);
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, bits), 10);
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, data_size), 12);
        assert_eq!(std::mem::offset_of!(LibRawProcessedImage, data), 16);
    }

    #[test]
    fn converts_rgb8_bitmap_to_binary_pnm() {
        let output = bitmap_to_pnm(&[1, 2, 3, 4, 5, 6], 2, 1, 3, 8).unwrap();
        assert_eq!(output, b"P6\n2 1\n255\n\x01\x02\x03\x04\x05\x06");
    }

    #[test]
    fn strips_alpha_and_writes_16_bit_samples_big_endian() {
        let samples = [
            0x1234_u16.to_ne_bytes(),
            0x5678_u16.to_ne_bytes(),
            0x9abc_u16.to_ne_bytes(),
            0xffff_u16.to_ne_bytes(),
        ]
        .concat();
        let output = bitmap_to_pnm(&samples, 1, 1, 4, 16).unwrap();
        assert_eq!(output, b"P6\n1 1\n65535\n\x12\x34\x56\x78\x9a\xbc");
    }

    #[test]
    fn rejects_truncated_bitmap() {
        let error = bitmap_to_pnm(&[1, 2], 1, 1, 3, 8).unwrap_err();
        assert!(error.to_string().contains("truncated"));
    }

    #[test]
    fn extracts_configured_raw_fixture() {
        let Ok(path) = std::env::var("BFD_TEST_RAW_PATH") else {
            return;
        };
        let started = Instant::now();
        let preview = extract_embedded_preview(Path::new(&path)).unwrap();
        let extraction = started.elapsed();
        let decoded = image::load_from_memory(&preview).unwrap();
        assert!(decoded.width() > 0);
        assert!(decoded.height() > 0);
        eprintln!(
            "LibRaw embedded preview: {}x{}, {} bytes, extraction {:.2?}",
            decoded.width(),
            decoded.height(),
            preview.len(),
            extraction
        );
    }
}
