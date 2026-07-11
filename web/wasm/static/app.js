import initWasm, { BrowserSession } from "./pkg/burst_wasm.js";

const RAW_EXTS = new Set([
  "3fr", "arw", "cr2", "cr3", "dcr", "dng", "erf", "fff", "iiq", "kdc", "mef",
  "mos", "mrw", "nef", "nrw", "orf", "pef", "raf", "raw", "rw2", "rwl", "sr2",
  "srf", "x3f",
]);
const IMAGE_EXTS = new Set([
  ...RAW_EXTS,
  "avif", "bmp", "gif", "heic", "heif", "jpeg", "jpg", "jxl", "png", "tif", "tiff", "webp",
]);
const SIDECAR_EXTS = new Set(["aae", "dop", "json", "pp3", "xmp"]);
const BROWSER_FIRST = ["jpg", "jpeg", "png", "webp", "avif", "bmp", "gif", "heic", "heif", "tif", "tiff"];
const PREVIEW_LONG_EDGE = 1280;
const runtimeOptions = new URLSearchParams(location.search);
const requestedDecodeConcurrency = Number(runtimeOptions.get("decode-concurrency"));
const DECODE_CONCURRENCY = Number.isFinite(requestedDecodeConcurrency) && requestedDecodeConcurrency > 0
  ? Math.max(1, Math.min(8, Math.floor(requestedDecodeConcurrency)))
  : Math.max(2, Math.min(4, Math.floor((navigator.hardwareConcurrency || 4) / 2)));
const WEB_CODECS_ENABLED = runtimeOptions.get("decode-backend") !== "image-bitmap";

const supportedLocales = new Set(["en", "zh-CN"]);
let messages = {};
let languageNames = {};

const elements = Object.fromEntries([
  "folderInput", "pickBtn", "emptyPickBtn", "emptyState", "progressView", "reviewView",
  "stageLabel", "progressDetail", "progressBar", "progressPercent", "cancelBtn", "stats",
  "searchInput", "filterSelect", "stacks", "saveBtn", "sourceLabel", "toast", "viewer",
  "viewerTitle", "viewerKeep", "viewerImage", "viewerLoading", "viewerError", "viewerViewport",
  "zoomOutBtn", "zoomInBtn", "fitBtn", "closeViewerBtn",
  "localeMenu", "saveDialog", "closeSaveBtn", "saveStats", "operationStatus", "destinationName",
  "chooseDestinationBtn", "posixTab", "powershellTab", "copyScriptBtn", "scriptCode",
  "exportJsonBtn", "restoreMovedBtn", "moveRejectedBtn", "confirmDialog", "confirmTitle",
  "confirmMessage", "confirmAction", "tutorialBtn", "aboutBtn", "aboutDialog", "aboutDialogTitle",
  "aboutDescription", "closeAboutBtn", "githubLink", "diagnosticsTitle", "diagnosticsList",
  "tutorialDialog", "tutorialLabel", "tutorialProgress", "tutorialDemoSource", "tutorialDemoReject",
  "tutorialDemoKeep", "tutorialDemoReview", "tutorialTitle", "tutorialBody", "tutorialSkip",
  "tutorialBack", "tutorialNext",
].map(id => [id, document.getElementById(id)]));

const queryLocale = new URLSearchParams(location.search).get("lang");
const requestedLocale = supportedLocales.has(queryLocale) ? queryLocale : null;
const defaultLocale = navigator.language.startsWith("zh") ? "zh-CN" : "en";
const storedLocale = localStorage.getItem("burst-locale");
const state = {
  locale: requestedLocale || (supportedLocales.has(storedLocale) ? storedLocale : defaultLocale),
  wasmReady: null,
  result: null,
  assets: new Map(),
  decisions: new Map(),
  expanded: new Set(),
  objectUrls: new Map(),
  scanToken: 0,
  sourceName: "",
  rawDecoder: null,
  rawDecodeQueue: null,
  decodeBackends: {},
  viewerAssetId: null,
  viewerPreviousFocus: null,
  viewerScale: 1,
  viewerX: 0,
  viewerY: 0,
  dragging: false,
  dragStart: null,
  sourceDirectoryHandle: null,
  fileHandles: new Map(),
  moveDestinationHandle: null,
  movedRecords: [],
  movedAssetIds: new Set(),
  activeScript: /win/i.test(navigator.userAgentData?.platform || navigator.platform || "") ? "powershell" : "posix",
  pendingOperation: null,
  tutorialStep: 0,
};

const tutorialSteps = [
  ["tutorialScanTitle", "tutorialScanBody"],
  ["tutorialSuggestionsTitle", "tutorialSuggestionsBody"],
  ["tutorialInspectTitle", "tutorialInspectBody"],
  ["tutorialMoveTitle", "tutorialMoveBody"],
];

async function loadLocaleCatalogs() {
  const catalogs = await Promise.all([...supportedLocales].map(async code => {
    const response = await fetch(`./locales/${code}.json`);
    if (!response.ok) throw new Error(`locale ${code}: HTTP ${response.status}`);
    const catalog = await response.json();
    return [code, catalog];
  }));
  messages = Object.fromEntries(catalogs.map(([code, catalog]) => [code, catalog.staticWeb]));
  languageNames = Object.fromEntries(catalogs.map(([code, catalog]) => [code, catalog.languageName]));
}

function t(key, values = {}) {
  const template = messages[state.locale]?.[key] ?? messages.en?.[key] ?? key;
  return String(template).replace(/\{([a-zA-Z0-9_]+)\}/g, (_, name) => String(values[name] ?? `{${name}}`));
}

function applyLocale() {
  document.documentElement.lang = state.locale;
  document.title = t("title");
  document.querySelector('meta[name="description"]').content = t("description");
  document.querySelectorAll("[data-i18n]").forEach(node => {
    node.textContent = t(node.dataset.i18n);
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach(node => {
    node.placeholder = t(node.dataset.i18nPlaceholder);
  });
  document.querySelectorAll("[data-i18n-aria]").forEach(node => {
    const label = t(node.dataset.i18nAria);
    node.setAttribute("aria-label", label);
    node.title = label;
  });
  document.querySelectorAll("[data-locale]").forEach(button => {
    button.textContent = languageNames[button.dataset.locale] || button.dataset.locale;
    button.classList.toggle("active", button.dataset.locale === state.locale);
  });
  elements.localeMenu.querySelector("summary").setAttribute("aria-label", t("language"));
  elements.zoomOutBtn.title = t("zoomOut");
  elements.zoomOutBtn.setAttribute("aria-label", t("zoomOut"));
  elements.zoomInBtn.title = t("zoomIn");
  elements.zoomInBtn.setAttribute("aria-label", t("zoomIn"));
  elements.sourceLabel.textContent = state.sourceName || t("browserMode");
  elements.filterSelect.setAttribute("aria-label", t("filter"));
  setButtonLabel(elements.tutorialBtn, t("tutorial"));
  setButtonLabel(elements.aboutBtn, t("about"));
  setButtonLabel(elements.closeAboutBtn, t("close"));
  elements.aboutDialogTitle.textContent = t("aboutTitle");
  elements.aboutDescription.textContent = t("aboutDescription");
  elements.githubLink.textContent = t("githubRepository");
  elements.diagnosticsTitle.textContent = t("diagnostics");
  elements.tutorialLabel.textContent = t("tutorial");
  elements.tutorialDemoSource.textContent = t("tutorialDemoSource");
  elements.tutorialDemoReject.textContent = t("tutorialDemoReject");
  elements.tutorialDemoKeep.textContent = t("tutorialDemoKeep");
  elements.tutorialDemoReview.textContent = t("tutorialDemoReview");
  elements.tutorialSkip.textContent = t("tutorialSkip");
  elements.tutorialBack.textContent = t("tutorialBack");
  renderTutorial();
  if (state.result) {
    renderReview();
    updateSaveDialog();
  }
}

function setButtonLabel(button, label) {
  button.title = label;
  button.setAttribute("aria-label", label);
}

function renderTutorial() {
  const [titleKey, bodyKey] = tutorialSteps[state.tutorialStep];
  elements.tutorialDialog.dataset.step = String(state.tutorialStep);
  elements.tutorialProgress.textContent = t("tutorialStep", {
    current: state.tutorialStep + 1,
    total: tutorialSteps.length,
  });
  elements.tutorialTitle.textContent = t(titleKey);
  elements.tutorialBody.textContent = t(bodyKey);
  elements.tutorialBack.disabled = state.tutorialStep === 0;
  elements.tutorialNext.textContent = t(
    state.tutorialStep === tutorialSteps.length - 1 ? "tutorialDone" : "tutorialNext"
  );
}

function openTutorial() {
  state.tutorialStep = 0;
  renderTutorial();
  elements.tutorialDialog.showModal();
}

function finishTutorial() {
  try { localStorage.setItem("burst-tutorial-wasm-v1", "complete"); } catch {}
  if (elements.tutorialDialog.open) elements.tutorialDialog.close();
}

function browserDiagnostics() {
  const brands = navigator.userAgentData?.brands
    ?.map(item => `${item.brand} ${item.version}`)
    .join(", ");
  return {
    browser: brands || navigator.userAgent,
    platform: navigator.userAgentData?.platform || navigator.platform || t("diagUnavailable"),
    language: navigator.language || t("diagUnavailable"),
    cpu: navigator.hardwareConcurrency || t("diagUnavailable"),
    memory: navigator.deviceMemory ? `${navigator.deviceMemory} GiB` : t("diagUnavailable"),
    isolation: String(window.crossOriginIsolated),
  };
}

async function openAbout() {
  let build = {};
  try {
    const response = await fetch("./build-info.json");
    if (response.ok) build = await response.json();
  } catch {}
  const browser = browserDiagnostics();
  renderDiagnostics([
    ["diagVersion", build.app_version],
    ["diagCommit", build.commit],
    ["diagRustc", build.rustc],
    ["diagCargo", build.cargo],
    ["diagWasmPack", build.wasm_pack],
    ["diagTarget", build.build_target],
    ["diagBrowser", browser.browser],
    ["diagPlatform", browser.platform],
    ["diagLanguage", browser.language],
    ["diagBrowserCpu", browser.cpu],
    ["diagBrowserMemory", browser.memory],
    ["diagIsolation", browser.isolation],
  ]);
  elements.aboutDialog.showModal();
}

function renderDiagnostics(rows) {
  elements.diagnosticsList.innerHTML = rows
    .filter(([, value]) => value !== undefined && value !== null && value !== "")
    .map(([key, value]) => `<dt>${escapeHtml(t(key))}</dt><dd>${escapeHtml(value)}</dd>`)
    .join("");
}

function setProgress(stageKey, fraction, detail = "") {
  const percent = Math.max(0, Math.min(100, Math.round(fraction * 100)));
  elements.stageLabel.textContent = t(stageKey);
  elements.progressDetail.textContent = detail;
  elements.progressBar.style.width = `${percent}%`;
  elements.progressPercent.textContent = `${percent}%`;
  elements.progressBar.parentElement.setAttribute("aria-valuenow", String(percent));
}

function showToast(message) {
  elements.toast.textContent = message;
  elements.toast.hidden = false;
  clearTimeout(showToast.timer);
  showToast.timer = setTimeout(() => { elements.toast.hidden = true; }, 4200);
}

function extension(name) {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

function withoutExtension(name) {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? name : name.slice(0, dot);
}

function stableId(value) {
  let hash = 0xcbf29ce484222325n;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= BigInt(value.charCodeAt(index));
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return `web-${hash.toString(16).padStart(16, "0")}`;
}

function groupSelectedFiles(fileList) {
  const groups = new Map();
  for (const file of fileList) {
    if (file.name.startsWith("._")) continue;
    const ext = extension(file.name);
    if (!IMAGE_EXTS.has(ext) && !SIDECAR_EXTS.has(ext)) continue;
    const relPath = file.webkitRelativePath || file.name;
    const slash = relPath.lastIndexOf("/");
    const directory = slash < 0 ? "" : relPath.slice(0, slash);
    const stem = withoutExtension(file.name);
    const key = `${directory.toLowerCase()}\0${stem.toLowerCase()}`;
    const group = groups.get(key) || { directory, stem, files: [] };
    group.files.push(file);
    groups.set(key, group);
  }

  return [...groups.values()].flatMap(group => {
    const imageFiles = group.files.filter(file => IMAGE_EXTS.has(extension(file.name)));
    if (!imageFiles.length) return [];
    imageFiles.sort((left, right) => representativePriority(left) - representativePriority(right));
    const representative = imageFiles[0];
    const relPath = representative.webkitRelativePath || representative.name;
    return [{
      id: stableId(relPath),
      relPath,
      representative,
      files: group.files,
      rawOnly: RAW_EXTS.has(extension(representative.name)),
    }];
  }).sort((left, right) => left.relPath.localeCompare(right.relPath, undefined, { numeric: true }));
}

function representativePriority(file) {
  const ext = extension(file.name);
  const browserIndex = BROWSER_FIRST.indexOf(ext);
  if (browserIndex >= 0) return browserIndex;
  if (RAW_EXTS.has(ext)) return 100;
  return 50;
}

async function ensureWasm() {
  if (!state.wasmReady) {
    state.wasmReady = initWasm();
  }
  await state.wasmReady;
}

async function scanFiles(fileList) {
  const benchmarkStarted = performance.now();
  const benchmarkStages = {
    discovery_ms: 0,
    wasm_initialization_ms: 0,
    decode_ms: 0,
    scoring_ms: 0,
    clustering_ms: 0,
    render_ms: 0,
  };
  const token = ++state.scanToken;
  resetResult();
  elements.emptyState.hidden = true;
  elements.reviewView.hidden = true;
  elements.progressView.hidden = false;
  elements.saveBtn.hidden = true;
  setProgress("discovering", 0.02);
  await nextPaint();

  const groups = groupSelectedFiles(fileList);
  benchmarkStages.discovery_ms = performance.now() - benchmarkStarted;
  if (!groups.length) {
    showEmpty(t("noImages"));
    return;
  }
  const firstPath = groups[0].relPath;
  state.sourceName = firstPath.includes("/") ? firstPath.split("/")[0] : t("browserMode");
  elements.sourceLabel.textContent = state.sourceName;

  setProgress("loadingEngine", 0.06);
  const wasmStarted = performance.now();
  try {
    await ensureWasm();
  } catch (error) {
    console.error(error);
    showEmpty(t("scanFailed"));
    return;
  }
  benchmarkStages.wasm_initialization_ms = performance.now() - wasmStarted;
  if (token !== state.scanToken) return;

  const session = new BrowserSession();
  const failed = [];
  state.decodeBackends = {};
  for (let offset = 0; offset < groups.length; offset += DECODE_CONCURRENCY) {
    if (token !== state.scanToken) {
      showEmpty(t("scanCancelled"));
      return;
    }
    const batch = groups.slice(offset, offset + DECODE_CONCURRENCY);
    const fraction = 0.08 + 0.80 * (offset / groups.length);
    setProgress("decoding", fraction, `${offset + 1}–${Math.min(groups.length, offset + batch.length)} / ${groups.length}`);
    await nextPaint();
    const decodeStarted = performance.now();
    const outcomes = await Promise.all(batch.map(async group => {
      try {
        return { group, decoded: await decodeGroup(group) };
      } catch (error) {
        return { group, error };
      }
    }));
    benchmarkStages.decode_ms += performance.now() - decodeStarted;

    for (const outcome of outcomes) {
      const { group, decoded, error } = outcome;
      if (error) {
        console.error(group.relPath, error);
        failed.push(group.relPath);
        continue;
      }
      if (token !== state.scanToken) {
        URL.revokeObjectURL(decoded.previewUrl);
        return;
      }
      const input = {
        id: group.id,
        rel_path: group.relPath,
        modified_ms: group.representative.lastModified || 0,
        capture_ms: decoded.captureMs,
        source_width: decoded.sourceWidth,
        source_height: decoded.sourceHeight,
        files: group.files.map(file => file.webkitRelativePath || file.name),
        metadata: decoded.metadata,
      };
      const scoringStarted = performance.now();
      session.add_rgba(input, decoded.width, decoded.height, decoded.rgba);
      benchmarkStages.scoring_ms += performance.now() - scoringStarted;
      state.decodeBackends[decoded.backend] = (state.decodeBackends[decoded.backend] || 0) + 1;
      state.assets.set(group.id, { ...group, previewUrl: decoded.previewUrl, rawPreview: group.rawOnly });
      state.objectUrls.set(group.id, decoded.previewUrl);
    }
  }

  if (token !== state.scanToken) return;
  if (session.len() === 0) {
    showEmpty(t("noImages"));
    return;
  }
  setProgress("grouping", 0.91);
  await nextPaint();
  const clusteringStarted = performance.now();
  try {
    state.result = session.finish(undefined);
  } catch (error) {
    console.error(error);
    showEmpty(t("scanFailed"));
    return;
  }
  benchmarkStages.clustering_ms = performance.now() - clusteringStarted;
  setProgress("rendering", 0.97);
  const renderStarted = performance.now();
  initializeReviewState();
  renderReview();
  benchmarkStages.render_ms = performance.now() - renderStarted;
  setProgress("complete", 1);
  await nextPaint();
  elements.progressView.hidden = true;
  elements.reviewView.hidden = false;
  elements.saveBtn.hidden = false;
  publishBenchmark(benchmarkStarted, benchmarkStages, groups.length, failed.length);
  if (failed.length) showToast(`${t("unsupportedSkipped")} (${failed.length})`);
}

function publishBenchmark(started, stages, selectedAssets, failedAssets) {
  const totalMs = performance.now() - started;
  const completedAssets = state.result?.assets.length || 0;
  const benchmark = {
    path: "wasm_static",
    selected_assets: selectedAssets,
    completed_assets: completedAssets,
    failed_assets: failedAssets,
    total_ms: totalMs,
    assets_per_second: totalMs > 0 ? completedAssets * 1000 / totalMs : 0,
    stages: stages,
    decode_backends: state.decodeBackends,
    decode_concurrency: DECODE_CONCURRENCY,
    assignments: (state.result?.assets || []).map(asset => ({
      filename: asset.rel_path.split("/").at(-1),
      stack_id: asset.stack_id,
      action: asset.action,
    })),
  };
  state.lastBenchmark = benchmark;
  window.__burstBenchmark = benchmark;
  document.documentElement.dataset.benchmarkComplete = "true";
  window.dispatchEvent(new CustomEvent("burst-benchmark-complete", { detail: benchmark }));
}

function decodeGroup(group) {
  if (!group.rawOnly) return decodeBrowserImage(group.representative);
  const previous = state.rawDecodeQueue || Promise.resolve();
  const task = previous.catch(() => {}).then(() => decodeRaw(group.representative));
  state.rawDecodeQueue = task;
  return task;
}

async function decodeBrowserImage(file) {
  if (WEB_CODECS_ENABLED && typeof ImageDecoder === "function" && file.type) {
    try {
      if (await ImageDecoder.isTypeSupported(file.type)) {
        return await decodeWithWebCodecs(file);
      }
    } catch (error) {
      console.debug("Scaled WebCodecs decode unavailable", error);
    }
  }
  return decodeWithImageBitmap(file);
}

async function decodeWithWebCodecs(file) {
  const probe = new ImageDecoder({ data: file.stream(), type: file.type, preferAnimation: false });
  await probe.tracks.ready;
  const track = probe.tracks.selectedTrack;
  if (!track) {
    probe.close();
    throw new Error(t("previewFailed"));
  }
  const sourceWidth = track.displayWidth || track.codedWidth;
  const sourceHeight = track.displayHeight || track.codedHeight;
  probe.close();
  const scale = Math.min(1, PREVIEW_LONG_EDGE / Math.max(sourceWidth, sourceHeight));
  const desiredWidth = Math.max(1, Math.round(sourceWidth * scale));
  const desiredHeight = Math.max(1, Math.round(sourceHeight * scale));
  const decoder = new ImageDecoder({
    data: file.stream(),
    type: file.type,
    preferAnimation: false,
    desiredWidth,
    desiredHeight,
  });
  try {
    const result = await decoder.decode({ frameIndex: 0, completeFramesOnly: true });
    const frame = result.image;
    try {
      const canvas = createCanvas(desiredWidth, desiredHeight);
      const context = canvas.getContext("2d", { alpha: false, willReadFrequently: true });
      context.drawImage(frame, 0, 0, desiredWidth, desiredHeight);
      const rgba = context.getImageData(0, 0, desiredWidth, desiredHeight).data;
      return {
        width: desiredWidth,
        height: desiredHeight,
        sourceWidth,
        sourceHeight,
        rgba,
        previewUrl: URL.createObjectURL(file),
        captureMs: null,
        metadata: {},
        backend: "webcodecs_scaled",
      };
    } finally {
      frame.close();
    }
  } finally {
    decoder.close();
  }
}

async function decodeWithImageBitmap(file) {
  let bitmap;
  try {
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
  } catch {
    bitmap = await createImageBitmap(file);
  }
  try {
    const sourceWidth = bitmap.width;
    const sourceHeight = bitmap.height;
    const canvas = drawScaled(bitmap, sourceWidth, sourceHeight, PREVIEW_LONG_EDGE);
    const context = canvas.getContext("2d", { willReadFrequently: true });
    const rgba = context.getImageData(0, 0, canvas.width, canvas.height).data;
    return {
      width: canvas.width,
      height: canvas.height,
      sourceWidth,
      sourceHeight,
      rgba,
      previewUrl: URL.createObjectURL(file),
      captureMs: null,
      metadata: {},
      backend: "image_bitmap",
    };
  } finally {
    bitmap.close();
  }
}

async function decodeRaw(file) {
  if (!crossOriginIsolated) throw new Error(t("rawIsolation"));
  if (!state.rawDecoder) {
    const module = await import("./vendor/libraw-wasm/index.js");
    state.rawDecoder = new module.default();
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  await state.rawDecoder.open(bytes, {
    halfSize: true,
    userQual: 1,
    outputBps: 8,
    useCameraWb: true,
    outputColor: 1,
  });
  const metadata = await state.rawDecoder.metadata(false) || {};
  const decoded = await state.rawDecoder.imageData();
  if (!decoded?.data || !decoded.width || !decoded.height) throw new Error(t("rawDecodeFailed"));
  const rgba = rawPixelsToRgba(decoded);
  const sourceCanvas = document.createElement("canvas");
  sourceCanvas.width = decoded.width;
  sourceCanvas.height = decoded.height;
  sourceCanvas.getContext("2d").putImageData(new ImageData(rgba, decoded.width, decoded.height), 0, 0);
  const preview = drawScaled(sourceCanvas, decoded.width, decoded.height, PREVIEW_LONG_EDGE);
  const previewContext = preview.getContext("2d", { willReadFrequently: true });
  const previewRgba = previewContext.getImageData(0, 0, preview.width, preview.height).data;
  const previewUrl = URL.createObjectURL(await canvasBlob(preview));
  return {
    width: preview.width,
    height: preview.height,
    sourceWidth: metadata.width || decoded.width,
    sourceHeight: metadata.height || decoded.height,
    rgba: previewRgba,
    previewUrl,
    captureMs: metadata.timestamp instanceof Date ? metadata.timestamp.getTime() : null,
    metadata: {
      iso: positiveNumber(metadata.iso_speed),
      aperture: positiveNumber(metadata.aperture),
      shutter: formatShutter(metadata.shutter),
      focal_length_mm: positiveNumber(metadata.focal_len),
    },
    backend: "libraw_wasm",
  };
}

function rawPixelsToRgba(decoded) {
  const colors = Math.max(1, decoded.colors || 3);
  const pixels = decoded.width * decoded.height;
  const rgba = new Uint8ClampedArray(pixels * 4);
  const highDepth = decoded.data instanceof Uint16Array || decoded.bits > 8;
  for (let index = 0; index < pixels; index += 1) {
    const source = index * colors;
    const target = index * 4;
    const sample = channel => {
      const value = decoded.data[source + Math.min(channel, colors - 1)] || 0;
      return highDepth ? value >>> 8 : value;
    };
    rgba[target] = sample(0);
    rgba[target + 1] = sample(1);
    rgba[target + 2] = sample(2);
    rgba[target + 3] = 255;
  }
  return rgba;
}

function drawScaled(source, width, height, longEdge) {
  const scale = Math.min(1, longEdge / Math.max(width, height));
  const canvas = createCanvas(
    Math.max(1, Math.round(width * scale)),
    Math.max(1, Math.round(height * scale))
  );
  const context = canvas.getContext("2d", { alpha: false });
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  context.drawImage(source, 0, 0, canvas.width, canvas.height);
  return canvas;
}

function createCanvas(width, height) {
  if (typeof OffscreenCanvas === "function") return new OffscreenCanvas(width, height);
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

function canvasBlob(canvas) {
  if (typeof canvas.convertToBlob === "function") {
    return canvas.convertToBlob({ type: "image/jpeg", quality: 0.88 });
  }
  return new Promise((resolve, reject) => {
    canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error(t("previewFailed"))), "image/jpeg", 0.88);
  });
}

function positiveNumber(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : null;
}

function formatShutter(value) {
  const seconds = positiveNumber(value);
  if (!seconds) return null;
  if (seconds >= 1) return `${seconds.toFixed(1).replace(/\.0$/, "")}s`;
  return `1/${Math.max(1, Math.round(1 / seconds))}s`;
}

function initializeReviewState() {
  state.decisions.clear();
  state.expanded.clear();
  for (const stack of state.result.stacks) {
    if (stack.asset_ids.length > 1) state.expanded.add(stack.id);
  }
}

function finalAction(asset) {
  return state.decisions.get(asset.id) || asset.action;
}

function setDecision(assetId, action) {
  const asset = state.result.assets.find(item => item.id === assetId);
  if (!asset) return;
  if (!action || action === asset.action) state.decisions.delete(assetId);
  else state.decisions.set(assetId, action);
  const stack = state.result.stacks.find(item => item.id === asset.stack_id);
  if (stack && stack.asset_ids.every(id => finalAction(state.result.assets.find(item => item.id === id)) === "keep")) {
    state.expanded.delete(stack.id);
  }
  renderReview();
  updateSaveDialog();
  renderMoveScripts();
  if (state.viewerAssetId === assetId) syncViewerKeep();
}

function renderReview() {
  if (!state.result) return;
  renderStats();
  const search = elements.searchInput.value.trim().toLowerCase();
  const filter = elements.filterSelect.value;
  const assetsById = new Map(state.result.assets.map(asset => [asset.id, asset]));
  const visibleStacks = state.result.stacks.map(stack => {
    const allAssets = stack.asset_ids.map(id => assetsById.get(id)).filter(Boolean);
    const assets = allAssets.filter(asset => {
      if (search && !asset.rel_path.toLowerCase().includes(search)) return false;
      const action = finalAction(asset);
      if (filter === "review" && action !== "review") return false;
      if (filter === "keep" && action !== "keep") return false;
      if (filter === "reject" && action !== "reject") return false;
      if (filter === "moved" && !state.movedAssetIds.has(asset.id)) return false;
      if (filter === "multi" && allAssets.length <= 1) return false;
      return true;
    });
    return { stack, allAssets, assets, expanded: state.expanded.has(stack.id) };
  }).filter(entry => entry.assets.length > 0);
  visibleStacks.sort((left, right) => Number(right.expanded) - Number(left.expanded) || left.stack.id - right.stack.id);
  elements.stacks.innerHTML = visibleStacks.map(renderStack).join("");
  elements.stacks.querySelectorAll("input[data-indeterminate='1']").forEach(input => { input.indeterminate = true; });
}

function renderStats() {
  const totals = { keep: 0, reject: 0, review: 0 };
  for (const asset of state.result.assets) totals[finalAction(asset)] += 1;
  const values = [
    ["images", state.result.summary.assets],
    ["bursts", state.result.summary.bursts],
    ["stacks", state.result.summary.stacks],
    ["keep", totals.keep],
    ["rejected", totals.reject],
    ["review", totals.review],
    ["moved", state.movedAssetIds.size],
    ["manual", state.decisions.size],
  ];
  elements.stats.innerHTML = values.map(([label, value]) => `<div class="stat"><span>${escapeHtml(t(label))}</span><b>${value}</b></div>`).join("");
}

function renderStack(entry) {
  const { stack, allAssets, assets, expanded } = entry;
  const keepCount = allAssets.filter(asset => finalAction(asset) === "keep").length;
  const stateLabel = expanded ? t("expanded") : t("collapsed");
  const count = searchActive() ? `${assets.length} / ${allAssets.length}` : t("frameCount", { count: allAssets.length });
  const diffKeys = metadataDifferences(allAssets);
  return `<section class="stack" data-stack="${stack.id}">
    <div class="stack-header">
      <div><h2>${escapeHtml(t("stackTitle", { burst: stack.burst_id, stack: stack.id }))}</h2></div>
      <div class="stack-meta">${escapeHtml(t("stackSummary", { count, state: stateLabel, keep: keepCount, confidence: Number(stack.similarity_confidence).toFixed(2) }))}</div>
      <button type="button" class="stack-toggle" data-toggle-stack="${stack.id}" title="${escapeHtml(stateLabel)}">${expanded ? "−" : "+"}</button>
    </div>
    <div class="frame-grid" ${expanded ? "" : "hidden"}>${assets.map(asset => renderFrame(asset, diffKeys)).join("")}</div>
  </section>`;
}

function renderFrame(asset, diffKeys) {
  const action = finalAction(asset);
  const moved = state.movedAssetIds.has(asset.id);
  const displayAction = moved ? "moved" : action;
  const checked = action === "keep" ? "checked" : "";
  const indeterminate = action === "review" ? "data-indeterminate=\"1\"" : "";
  const source = state.assets.get(asset.id);
  const preview = source?.previewUrl || "";
  const manual = state.decisions.has(asset.id);
  return `<article class="frame ${action} ${moved ? "moved" : ""}" data-asset="${escapeHtml(asset.id)}">
    <button type="button" class="thumbnail" data-open="${escapeHtml(asset.id)}" aria-label="${escapeHtml(asset.rel_path)}">
      ${preview ? `<img src="${escapeHtml(preview)}" loading="lazy" alt="">` : ""}
      <span class="badge ${displayAction}">${escapeHtml(t(displayAction === "review" ? "review" : displayAction === "reject" ? "rejected" : displayAction))}</span>
    </button>
    <div class="frame-body">
      <label class="keep-control"><input type="checkbox" data-decision="${escapeHtml(asset.id)}" ${checked} ${indeterminate}> ${escapeHtml(t("keep"))}</label>
      <div class="filename">${escapeHtml(asset.rel_path)}</div>
      <div class="exif">${metadataHtml(asset, diffKeys)}</div>
      <div class="reason">${escapeHtml(moved ? t("movedReason") : t(asset.reason_key))}</div>
      <details><summary>${escapeHtml(t("why"))}</summary><ul>
        <li>${escapeHtml(t("rank", { rank: asset.rank, score: Number(asset.score).toFixed(3) }))}</li>
        <li>${escapeHtml(t("sharpness", { whole: Number(asset.metrics.sharpness).toFixed(1), subject: Number(asset.metrics.subject_sharpness).toFixed(1) }))}</li>
        <li>${escapeHtml(t("similarity", { distance: Number(asset.similarity.nearest_distance).toFixed(3), confidence: Number(asset.similarity.duplicate_confidence).toFixed(2) }))}</li>
        <li>${escapeHtml(t("dimensions", { width: asset.source_width, height: asset.source_height }))}</li>
      </ul></details>
      ${manual ? `<button type="button" class="reset" data-reset="${escapeHtml(asset.id)}">${escapeHtml(t("reset"))}</button>` : ""}
    </div>
  </article>`;
}

function metadataDifferences(assets) {
  const keys = ["iso", "aperture", "shutter", "focal_length_mm", "focal_length_35mm"];
  return new Set(keys.filter(key => new Set(assets.map(asset => asset.metadata?.[key]).filter(value => value !== null && value !== undefined && value !== "")).size > 1));
}

function metadataHtml(asset, diffKeys) {
  const metadata = asset.metadata || {};
  const fields = [
    ["iso", metadata.iso ? `ISO ${metadata.iso}` : ""],
    ["aperture", metadata.aperture ? `f/${Number(metadata.aperture).toFixed(1).replace(/\.0$/, "")}` : ""],
    ["shutter", metadata.shutter || ""],
    ["focal_length_mm", metadata.focal_length_mm ? `${Number(metadata.focal_length_mm).toFixed(1).replace(/\.0$/, "")}mm` : ""],
    ["focal_length_35mm", metadata.focal_length_35mm ? `${metadata.focal_length_35mm}mm eq` : ""],
  ].filter(([, value]) => value);
  if (!fields.length) return `<span class="chip">${escapeHtml(t("exifUnavailable"))}</span>`;
  return fields.map(([key, value]) => `<span class="chip ${diffKeys.has(key) ? "diff" : ""}">${escapeHtml(value)}</span>`).join("");
}

function searchActive() {
  return elements.searchInput.value.trim() !== "" || elements.filterSelect.value !== "all";
}

function openViewer(assetId, trigger) {
  const asset = state.result.assets.find(item => item.id === assetId);
  if (!asset) return;
  state.viewerPreviousFocus = trigger || document.activeElement;
  state.viewerAssetId = assetId;
  elements.viewer.hidden = false;
  elements.viewer.setAttribute("aria-hidden", "false");
  elements.viewerTitle.textContent = `${asset.rel_path}${state.assets.get(assetId)?.rawPreview ? ` · ${t("rawPreview")}` : ""}`;
  elements.viewerLoading.hidden = false;
  elements.viewerError.hidden = true;
  elements.viewerImage.hidden = true;
  syncViewerKeep();
  const url = state.objectUrls.get(assetId);
  if (!url) {
    elements.viewerLoading.hidden = true;
    elements.viewerError.textContent = t("sourceUnavailable");
    elements.viewerError.hidden = false;
  } else {
    elements.viewerImage.onload = () => {
      elements.viewerLoading.hidden = true;
      elements.viewerImage.hidden = false;
      fitViewer();
    };
    elements.viewerImage.onerror = () => {
      elements.viewerLoading.hidden = true;
      elements.viewerError.textContent = t("sourceUnavailable");
      elements.viewerError.hidden = false;
    };
    elements.viewerImage.src = url;
  }
  elements.viewer.focus({ preventScroll: true });
}

function closeViewer() {
  elements.viewer.hidden = true;
  elements.viewer.setAttribute("aria-hidden", "true");
  elements.viewerImage.removeAttribute("src");
  state.viewerAssetId = null;
  if (state.viewerPreviousFocus?.isConnected) state.viewerPreviousFocus.focus({ preventScroll: true });
  state.viewerPreviousFocus = null;
}

function syncViewerKeep() {
  const asset = state.result?.assets.find(item => item.id === state.viewerAssetId);
  if (!asset) return;
  const action = finalAction(asset);
  elements.viewerKeep.checked = action === "keep";
  elements.viewerKeep.indeterminate = action === "review";
}

function adjacentViewer(delta) {
  const asset = state.result.assets.find(item => item.id === state.viewerAssetId);
  const stack = state.result.stacks.find(item => item.id === asset?.stack_id);
  if (!stack) return;
  const index = stack.asset_ids.indexOf(state.viewerAssetId);
  const next = Math.max(0, Math.min(stack.asset_ids.length - 1, index + delta));
  if (next !== index) openViewer(stack.asset_ids[next], state.viewerPreviousFocus);
}

function fitViewer() {
  if (!elements.viewerImage.naturalWidth) return;
  const viewport = elements.viewerViewport.getBoundingClientRect();
  state.viewerScale = Math.min(viewport.width / elements.viewerImage.naturalWidth, viewport.height / elements.viewerImage.naturalHeight, 1);
  state.viewerX = 0;
  state.viewerY = 0;
  applyViewerTransform();
}

function zoomViewer(factor) {
  state.viewerScale = Math.max(0.05, Math.min(8, state.viewerScale * factor));
  applyViewerTransform();
}

function applyViewerTransform() {
  elements.viewerImage.style.transform = `translate(calc(-50% + ${state.viewerX}px), calc(-50% + ${state.viewerY}px)) scale(${state.viewerScale})`;
}

function saveReview() {
  if (!state.result) return;
  updateSaveDialog();
  renderMoveScripts();
  elements.operationStatus.hidden = true;
  elements.saveDialog.showModal();
}

function reviewPayload() {
  const payload = {
    version: 1,
    created_at: new Date().toISOString(),
    source: state.sourceName,
    move_status: {
      active_asset_ids: [...state.movedAssetIds],
      active_files: state.movedRecords.length,
      destinations: state.moveDestinationHandle ? [state.moveDestinationHandle.name] : [],
    },
    decisions: state.result.assets.map(asset => ({
      id: asset.id,
      path: asset.rel_path,
      files: asset.files,
      suggestion: asset.action,
      decision: finalAction(asset),
      burst_id: asset.burst_id,
      stack_id: asset.stack_id,
    })),
  };
  return payload;
}

function exportReviewJson() {
  const payload = reviewPayload();
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "burst-review.json";
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  showToast(t("saved"));
}

function reviewTotals() {
  const totals = { keep: 0, reject: 0, review: 0, moved: state.movedAssetIds.size };
  for (const asset of state.result.assets) totals[finalAction(asset)] += 1;
  return totals;
}

function movableAssets() {
  return state.result.assets.filter(asset => finalAction(asset) === "reject" && !state.movedAssetIds.has(asset.id));
}

function updateSaveDialog() {
  if (!state.result) return;
  const totals = reviewTotals();
  elements.saveStats.innerHTML = ["keep", "rejected", "review", "moved"].map(key => {
    const value = key === "rejected" ? totals.reject : totals[key];
    return `<div class="save-stat"><span>${escapeHtml(t(key))}</span><b>${value}</b></div>`;
  }).join("");
  const rejects = movableAssets().length;
  elements.moveRejectedBtn.textContent = rejects ? t("moveRejects", { count: rejects }) : t("noRejects");
  elements.moveRejectedBtn.disabled = rejects === 0;
  elements.restoreMovedBtn.hidden = state.movedAssetIds.size === 0;
  elements.destinationName.textContent = state.moveDestinationHandle?.name || t("chooseMoveDestination");
}

function renderMoveScripts() {
  const scripts = generateMoveScripts();
  elements.posixTab.setAttribute("aria-selected", String(state.activeScript === "posix"));
  elements.powershellTab.setAttribute("aria-selected", String(state.activeScript === "powershell"));
  elements.scriptCode.textContent = scripts[state.activeScript];
}

function generateMoveScripts() {
  const rejected = movableAssets();
  let posix = `#!/usr/bin/env bash\nset -euo pipefail\nSOURCE_ROOT=${shellQuote(`/path/to/${state.sourceName || "photos"}`)}\nDESTINATION=${shellQuote("/path/to/Burst Rejects")}\nmkdir -p \"$DESTINATION\"\n\n`;
  let powershell = `param(\n  [string]$SourceRoot = ${powerShellQuote(`C:\\path\\to\\${state.sourceName || "photos"}`)},\n  [string]$Destination = ${powerShellQuote("C:\\path\\to\\Burst Rejects")}\n)\n$ErrorActionPreference = 'Stop'\nNew-Item -ItemType Directory -Force -Path $Destination | Out-Null\n\n`;
  for (const asset of rejected) {
    posix += `# ${asset.id}\n`;
    powershell += `# ${asset.id}\n`;
    const paths = asset.files.map(pathWithoutRoot);
    for (const [index, relative] of paths.entries()) {
      posix += `source_${index}=\"$SOURCE_ROOT\"/${shellQuote(relative)}\ntarget_${index}=\"$DESTINATION\"/${shellQuote(relative)}\n`;
    }
    posix += "asset_ready=1\n";
    for (const index of paths.keys()) {
      posix += `if [ ! -f \"$source_${index}\" ]; then printf 'Source unavailable: %s\\n' \"$source_${index}\" >&2; asset_ready=0; fi\nif [ -e \"$target_${index}\" ]; then printf 'Destination exists: %s\\n' \"$target_${index}\" >&2; asset_ready=0; fi\n`;
    }
    const cleanupTargets = paths.map((_, index) => `\"$target_${index}\"`).join(" ");
    posix += "if [ \"$asset_ready\" -eq 1 ]; then\n";
    for (const index of paths.keys()) {
      posix += `  mkdir -p \"$(dirname \"$target_${index}\")\"\n  if ! cp -p -- \"$source_${index}\" \"$target_${index}\"; then rm -f -- ${cleanupTargets}; exit 1; fi\n  if [ \"$(wc -c < \"$source_${index}\")\" -ne \"$(wc -c < \"$target_${index}\")\" ]; then rm -f -- ${cleanupTargets}; exit 1; fi\n`;
    }
    for (const index of paths.keys()) {
      posix += `  if ! rm -- \"$source_${index}\"; then\n`;
      for (let restored = 0; restored < index; restored += 1) {
        posix += `    cp -p -- \"$target_${restored}\" \"$source_${restored}\"\n    [ \"$(wc -c < \"$source_${restored}\")\" -eq \"$(wc -c < \"$target_${restored}\")\" ]\n`;
      }
      posix += `    rm -f -- ${cleanupTargets}\n    exit 1\n  fi\n`;
    }
    posix += "fi\n\n";

    powershell += "$pairs = @(\n";
    for (const relative of paths) {
      const windowsRelative = relative.replaceAll("/", "\\");
      powershell += `  [pscustomobject]@{ Source = Join-Path $SourceRoot ${powerShellQuote(windowsRelative)}; Target = Join-Path $Destination ${powerShellQuote(windowsRelative)} }\n`;
    }
    powershell += `)\n$assetReady = $true\nforeach ($pair in $pairs) { if (-not (Test-Path -LiteralPath $pair.Source -PathType Leaf)) { Write-Warning \"Source unavailable: $($pair.Source)\"; $assetReady = $false }; if (Test-Path -LiteralPath $pair.Target) { Write-Warning \"Destination exists: $($pair.Target)\"; $assetReady = $false } }\nif ($assetReady) {\n  $copied = @()\n  $removed = @()\n  try {\n    foreach ($pair in $pairs) {\n      New-Item -ItemType Directory -Force -Path (Split-Path -Parent $pair.Target) | Out-Null\n      Copy-Item -LiteralPath $pair.Source -Destination $pair.Target\n      $copied += $pair\n      if ((Get-Item -LiteralPath $pair.Source).Length -ne (Get-Item -LiteralPath $pair.Target).Length) { throw \"Copy verification failed: $($pair.Source)\" }\n    }\n    foreach ($pair in $pairs) { Remove-Item -LiteralPath $pair.Source; $removed += $pair }\n  } catch {\n    foreach ($pair in $removed) { if (-not (Test-Path -LiteralPath $pair.Source)) { Copy-Item -LiteralPath $pair.Target -Destination $pair.Source } }\n    foreach ($pair in $copied) { if ((Test-Path -LiteralPath $pair.Source) -and (Test-Path -LiteralPath $pair.Target)) { Remove-Item -LiteralPath $pair.Target -Force } }\n    throw\n  }\n}\n\n`;
  }
  return { posix, powershell };
}

function shellQuote(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

function powerShellQuote(value) {
  return `'${String(value).replaceAll("'", "''")}'`;
}

function pathWithoutRoot(path) {
  const parts = String(path).split("/").filter(Boolean);
  if (parts[0] === state.sourceName) parts.shift();
  return parts.filter(part => part !== "." && part !== "..").join("/");
}

async function chooseMoveDestination() {
  if (typeof window.showDirectoryPicker !== "function") {
    setOperationStatus(t("fileAccessUnsupported"), true);
    return false;
  }
  try {
    state.moveDestinationHandle = await window.showDirectoryPicker({ mode: "readwrite" });
    updateSaveDialog();
    return true;
  } catch (error) {
    if (error.name !== "AbortError") setOperationStatus(friendlyFileError(error), true);
    return false;
  }
}

async function requestFileOperation(operation) {
  if (!state.sourceDirectoryHandle || state.fileHandles.size === 0) {
    setOperationStatus(t("writableSourceRequired"), true);
    return;
  }
  if (operation === "move" && !state.moveDestinationHandle && !await chooseMoveDestination()) return;
  state.pendingOperation = operation;
  const count = operation === "move" ? movableAssets().length : state.movedAssetIds.size;
  elements.confirmTitle.textContent = t(operation === "move" ? "moveConfirmTitle" : "restoreConfirmTitle");
  elements.confirmMessage.textContent = t(operation === "move" ? "moveConfirm" : "restoreConfirm", { count });
  elements.confirmAction.textContent = t(operation === "move" ? "move" : "restore");
  elements.confirmAction.className = operation === "move" ? "danger" : "primary";
  elements.confirmDialog.returnValue = "";
  elements.confirmDialog.showModal();
}

async function runFileOperation(operation) {
  elements.moveRejectedBtn.disabled = true;
  elements.restoreMovedBtn.disabled = true;
  try {
    if (operation === "move") await moveRejectedAssets();
    else await restoreMovedAssets();
  } catch (error) {
    setOperationStatus(friendlyFileError(error), true);
  } finally {
    updateSaveDialog();
    renderReview();
    renderMoveScripts();
    elements.restoreMovedBtn.disabled = false;
  }
}

async function moveRejectedAssets() {
  const assets = movableAssets();
  const stamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "_");
  const rejectRoot = await state.moveDestinationHandle.getDirectoryHandle(`Burst Rejects ${stamp}`, { create: true });
  let movedFiles = 0;
  let movedAssets = 0;
  const failures = [];
  for (const asset of assets) {
    const infos = asset.files.map(path => state.fileHandles.get(path));
    if (infos.some(info => !info)) {
      failures.push(asset.rel_path);
      continue;
    }
    try {
      const records = await transferAssetToDestination(asset.id, infos, rejectRoot);
      state.movedRecords.push(...records);
      state.movedAssetIds.add(asset.id);
      movedFiles += records.length;
      movedAssets += 1;
    } catch (error) {
      failures.push(`${asset.rel_path}: ${friendlyFileError(error)}`);
    }
  }
  setOperationStatus(t("movedResult", { files: movedFiles, assets: movedAssets, failed: failures.length }), failures.length > 0);
}

async function transferAssetToDestination(assetId, infos, rejectRoot) {
  const copied = [];
  try {
    for (const info of infos) {
      const sourceFile = await info.fileHandle.getFile();
      const relative = pathWithoutRoot(info.relPath);
      const target = await createTargetHandle(rejectRoot, relative);
      await copyFileToHandle(sourceFile, target.fileHandle);
      copied.push({
        assetId,
        size: sourceFile.size,
        originalParent: info.parentHandle,
        originalName: info.name,
        originalPath: info.relPath,
        destinationParent: target.parentHandle,
        destinationName: target.name,
        destinationHandle: target.fileHandle,
      });
    }
  } catch (error) {
    await removeDestinationCopies(copied);
    throw error;
  }

  const removed = [];
  try {
    for (const record of copied) {
      await record.originalParent.removeEntry(record.originalName);
      removed.push(record);
    }
  } catch (error) {
    for (const record of removed) await restoreRecordCopy(record);
    await removeDestinationCopies(copied);
    throw error;
  }
  return copied;
}

async function restoreMovedAssets() {
  const grouped = new Map();
  for (const record of state.movedRecords) {
    const records = grouped.get(record.assetId) || [];
    records.push(record);
    grouped.set(record.assetId, records);
  }
  let restoredFiles = 0;
  let restoredAssets = 0;
  const restoredIds = new Set();
  const failures = [];
  for (const [assetId, records] of grouped) {
    const restored = [];
    try {
      for (const record of records) {
        if (await entryExists(record.originalParent, record.originalName)) {
          throw new Error(`${t("originalPathOccupied")}: ${record.originalPath}`);
        }
        await restoreRecordCopy(record);
        restored.push(record);
      }
      for (const record of records) {
        await record.destinationParent.removeEntry(record.destinationName);
      }
      restoredFiles += records.length;
      restoredAssets += 1;
      restoredIds.add(assetId);
      state.movedAssetIds.delete(assetId);
    } catch (error) {
      for (const record of restored) {
        try {
          if (await entryExists(record.destinationParent, record.destinationName)) {
            await record.originalParent.removeEntry(record.originalName);
          }
        } catch {}
      }
      failures.push(friendlyFileError(error));
    }
  }
  state.movedRecords = state.movedRecords.filter(record => !restoredIds.has(record.assetId));
  setOperationStatus(t("restoredResult", { files: restoredFiles, assets: restoredAssets, failed: failures.length }), failures.length > 0);
}

async function createTargetHandle(root, relativePath) {
  const parts = relativePath.split("/").filter(Boolean);
  const name = parts.pop();
  let parentHandle = root;
  for (const part of parts) parentHandle = await parentHandle.getDirectoryHandle(part, { create: true });
  if (await entryExists(parentHandle, name)) throw new Error(`${t("destinationExists")}: ${relativePath}`);
  const fileHandle = await parentHandle.getFileHandle(name, { create: true });
  return { parentHandle, fileHandle, name };
}

async function copyFileToHandle(sourceFile, targetHandle) {
  const writable = await targetHandle.createWritable();
  try {
    await writable.write(sourceFile);
  } finally {
    await writable.close();
  }
  const copied = await targetHandle.getFile();
  if (copied.size !== sourceFile.size) throw new Error(t("copyVerificationFailed"));
}

async function restoreRecordCopy(record) {
  const sourceFile = await record.destinationHandle.getFile();
  const originalHandle = await record.originalParent.getFileHandle(record.originalName, { create: true });
  await copyFileToHandle(sourceFile, originalHandle);
  const restored = await originalHandle.getFile();
  if (restored.size !== record.size) throw new Error(t("copyVerificationFailed"));
}

async function removeDestinationCopies(records) {
  for (const record of records) {
    try { await record.destinationParent.removeEntry(record.destinationName); } catch {}
  }
}

async function entryExists(parent, name) {
  try {
    await parent.getFileHandle(name);
    return true;
  } catch (error) {
    if (error.name === "NotFoundError") return false;
    throw error;
  }
}

function setOperationStatus(message, error = false) {
  elements.operationStatus.textContent = message;
  elements.operationStatus.hidden = false;
  elements.operationStatus.classList.toggle("error", error);
}

function friendlyFileError(error) {
  if (error?.name === "NotFoundError") return t("sourceUnavailableMove");
  if (error?.name === "NotAllowedError") return t("permissionRequired");
  return error?.message || String(error);
}

function resetResult() {
  if (!elements.viewer.hidden) closeViewer();
  for (const url of state.objectUrls.values()) URL.revokeObjectURL(url);
  state.objectUrls.clear();
  state.assets.clear();
  state.result = null;
  state.decisions.clear();
  state.expanded.clear();
  state.movedRecords = [];
  state.movedAssetIds.clear();
  state.moveDestinationHandle = null;
  elements.searchInput.value = "";
  elements.filterSelect.value = "all";
}

async function chooseSourceFolder() {
  if (typeof window.showDirectoryPicker !== "function") {
    elements.folderInput.click();
    return;
  }
  try {
    const handle = await window.showDirectoryPicker({ mode: "readwrite" });
    state.sourceDirectoryHandle = handle;
    state.fileHandles = new Map();
    const files = await collectDirectoryFiles(handle);
    if (files.length) scanFiles(files);
    else showEmpty(t("noImages"));
  } catch (error) {
    if (error.name !== "AbortError") showToast(friendlyFileError(error));
  }
}

async function collectDirectoryFiles(rootHandle) {
  const files = [];
  async function visit(directoryHandle, relativeDirectory) {
    for await (const [name, handle] of directoryHandle.entries()) {
      if (name.startsWith("._") || name === ".DS_Store") continue;
      const relative = relativeDirectory ? `${relativeDirectory}/${name}` : name;
      if (handle.kind === "directory") {
        await visit(handle, relative);
        continue;
      }
      const file = await handle.getFile();
      const fullPath = `${rootHandle.name}/${relative}`;
      Object.defineProperty(file, "webkitRelativePath", { value: fullPath, configurable: true });
      state.fileHandles.set(fullPath, {
        parentHandle: directoryHandle,
        fileHandle: handle,
        name,
        relPath: fullPath,
      });
      files.push(file);
    }
  }
  await visit(rootHandle, "");
  return files;
}

function showEmpty(message) {
  elements.progressView.hidden = true;
  elements.reviewView.hidden = true;
  elements.emptyState.hidden = false;
  elements.saveBtn.hidden = true;
  if (message) showToast(message);
}

function nextPaint() {
  return new Promise(resolve => requestAnimationFrame(() => setTimeout(resolve, 0)));
}

async function syntheticFixture() {
  const files = [];
  for (let index = 0; index < 12; index += 1) {
    const canvas = document.createElement("canvas");
    canvas.width = 640;
    canvas.height = 480;
    const context = canvas.getContext("2d");
    context.fillStyle = "#a5bed2";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.fillStyle = "#20272b";
    const x = 180 + index * 24;
    const vertical = index >= 6;
    if (vertical) {
      context.fillRect(x - 8, 188, 16, 104);
      context.fillRect(x - 34, 224, 68, 24);
    } else {
      context.fillRect(x - 58, 228, 116, 20);
      context.fillRect(x - 18, 202, 36, 72);
    }
    const blob = await canvasBlob(canvas);
    const name = `frame_${String(index + 1).padStart(4, "0")}.jpg`;
    const file = new File([blob], name, { type: "image/jpeg", lastModified: 1000 + index * 100 });
    Object.defineProperty(file, "webkitRelativePath", { value: `synthetic_burst/${name}` });
    files.push(file);
  }
  return files;
}

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>'"]/g, character => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;",
  })[character]);
}

elements.pickBtn.addEventListener("click", chooseSourceFolder);
elements.emptyPickBtn.addEventListener("click", chooseSourceFolder);
elements.tutorialBtn.addEventListener("click", openTutorial);
elements.aboutBtn.addEventListener("click", openAbout);
elements.closeAboutBtn.addEventListener("click", () => elements.aboutDialog.close());
elements.tutorialSkip.addEventListener("click", finishTutorial);
elements.tutorialBack.addEventListener("click", () => {
  if (state.tutorialStep > 0) state.tutorialStep -= 1;
  renderTutorial();
});
elements.tutorialNext.addEventListener("click", () => {
  if (state.tutorialStep === tutorialSteps.length - 1) {
    finishTutorial();
    return;
  }
  state.tutorialStep += 1;
  renderTutorial();
});
elements.tutorialDialog.addEventListener("cancel", () => {
  try { localStorage.setItem("burst-tutorial-wasm-v1", "complete"); } catch {}
});
elements.folderInput.addEventListener("change", event => {
  const files = Array.from(event.target.files || []);
  event.target.value = "";
  state.sourceDirectoryHandle = null;
  state.fileHandles = new Map();
  if (files.length) scanFiles(files);
});
elements.cancelBtn.addEventListener("click", () => {
  state.scanToken += 1;
  resetResult();
  showEmpty(t("scanCancelled"));
});
elements.saveBtn.addEventListener("click", saveReview);
elements.closeSaveBtn.addEventListener("click", () => elements.saveDialog.close());
elements.chooseDestinationBtn.addEventListener("click", chooseMoveDestination);
elements.posixTab.addEventListener("click", () => {
  state.activeScript = "posix";
  renderMoveScripts();
});
elements.powershellTab.addEventListener("click", () => {
  state.activeScript = "powershell";
  renderMoveScripts();
});
elements.copyScriptBtn.addEventListener("click", async () => {
  await navigator.clipboard.writeText(elements.scriptCode.textContent);
  showToast(t("scriptCopied"));
});
elements.exportJsonBtn.addEventListener("click", exportReviewJson);
elements.moveRejectedBtn.addEventListener("click", () => requestFileOperation("move"));
elements.restoreMovedBtn.addEventListener("click", () => requestFileOperation("restore"));
elements.confirmDialog.addEventListener("close", () => {
  if (elements.confirmDialog.returnValue === "confirm" && state.pendingOperation) {
    runFileOperation(state.pendingOperation);
  }
  state.pendingOperation = null;
});
elements.searchInput.addEventListener("input", renderReview);
elements.filterSelect.addEventListener("change", renderReview);
document.querySelectorAll("[data-locale]").forEach(button => {
  button.addEventListener("click", () => {
    state.locale = button.dataset.locale;
    localStorage.setItem("burst-locale", state.locale);
    elements.localeMenu.open = false;
    applyLocale();
  });
});
elements.stacks.addEventListener("click", event => {
  const toggle = event.target.closest("[data-toggle-stack]");
  if (toggle) {
    const id = Number(toggle.dataset.toggleStack);
    if (state.expanded.has(id)) state.expanded.delete(id);
    else state.expanded.add(id);
    renderReview();
    return;
  }
  const open = event.target.closest("[data-open]");
  if (open) {
    openViewer(open.dataset.open, open);
    return;
  }
  const reset = event.target.closest("[data-reset]");
  if (reset) setDecision(reset.dataset.reset, null);
});
elements.stacks.addEventListener("change", event => {
  const input = event.target.closest("[data-decision]");
  if (!input) return;
  input.indeterminate = false;
  setDecision(input.dataset.decision, input.checked ? "keep" : "reject");
});
elements.viewerKeep.addEventListener("change", event => {
  event.target.indeterminate = false;
  setDecision(state.viewerAssetId, event.target.checked ? "keep" : "reject");
});
elements.closeViewerBtn.addEventListener("click", closeViewer);
elements.zoomOutBtn.addEventListener("click", () => zoomViewer(0.8));
elements.zoomInBtn.addEventListener("click", () => zoomViewer(1.25));
elements.fitBtn.addEventListener("click", fitViewer);
elements.viewerViewport.addEventListener("wheel", event => {
  event.preventDefault();
  zoomViewer(event.deltaY < 0 ? 1.12 : 0.89);
}, { passive: false });
elements.viewerViewport.addEventListener("pointerdown", event => {
  state.dragging = true;
  state.dragStart = { x: event.clientX, y: event.clientY, viewerX: state.viewerX, viewerY: state.viewerY };
  elements.viewerViewport.classList.add("dragging");
  elements.viewerViewport.setPointerCapture(event.pointerId);
});
elements.viewerViewport.addEventListener("pointermove", event => {
  if (!state.dragging) return;
  state.viewerX = state.dragStart.viewerX + event.clientX - state.dragStart.x;
  state.viewerY = state.dragStart.viewerY + event.clientY - state.dragStart.y;
  applyViewerTransform();
});
elements.viewerViewport.addEventListener("pointerup", event => {
  state.dragging = false;
  state.dragStart = null;
  elements.viewerViewport.classList.remove("dragging");
  elements.viewerViewport.releasePointerCapture(event.pointerId);
});
document.addEventListener("keydown", event => {
  if (elements.viewer.hidden) return;
  if (event.key === "Escape") closeViewer();
  if (event.key === "ArrowLeft") { event.preventDefault(); adjacentViewer(-1); }
  if (event.key === "ArrowRight") { event.preventDefault(); adjacentViewer(1); }
});

async function initialize() {
  await loadLocaleCatalogs();
  applyLocale();
  document.documentElement.dataset.crossOriginIsolated = String(crossOriginIsolated);
  const testFixture = new URLSearchParams(location.search).get("test-fixture");
  if (testFixture === "synthetic" && ["127.0.0.1", "localhost"].includes(location.hostname)) {
    await scanFiles(await syntheticFixture());
  }
  let tutorialComplete = false;
  try { tutorialComplete = localStorage.getItem("burst-tutorial-wasm-v1") === "complete"; } catch {}
  if (!tutorialComplete) openTutorial();
}

initialize().catch(error => {
  console.error(error);
  document.documentElement.dataset.initialization = "failed";
});
