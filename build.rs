fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let apple = target.contains("apple-darwin");

    if apple && std::env::var_os("CARGO_FEATURE_METAL_ACCEL").is_some() {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    if apple && std::env::var_os("CARGO_FEATURE_MACOS_VISION").is_some() {
        println!("cargo:rustc-link-lib=framework=Vision");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
    }
}
