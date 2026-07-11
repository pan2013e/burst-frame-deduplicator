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

const messages = {
  en: {
    title: "Burst Frame Deduplicator",
    browserMode: "Browser mode",
    chooseFolder: "Choose photo folder",
    localOnly: "Photos stay on this device",
    saveReview: "Save review",
    cancel: "Cancel",
    findFilename: "Find filename",
    filter: "Filter",
    allFrames: "All frames",
    needsReview: "Needs review",
    kept: "Kept",
    rejected: "Rejected",
    multiStacks: "Multi-frame stacks",
    keep: "Keep",
    fit: "Fit",
    close: "Close",
    loading: "Loading preview",
    images: "Images",
    bursts: "Bursts",
    stacks: "Stacks",
    review: "Review",
    manual: "Manual edits",
    discovering: "Discovering photos",
    loadingEngine: "Loading local analysis engine",
    decoding: "Decoding and analyzing previews",
    grouping: "Grouping bursts and near-duplicates",
    rendering: "Preparing review",
    complete: "Ready to review",
    noImages: "No supported photos were found in this folder.",
    scanCancelled: "Scan cancelled.",
    scanFailed: "The browser scan could not be completed.",
    rawIsolation: "RAW decoding needs cross-origin isolation. Reload this page after the service worker activates.",
    rawDecodeFailed: "RAW preview could not be decoded.",
    previewFailed: "Preview could not be decoded.",
    unsupportedSkipped: "Some files could not be decoded and were skipped.",
    distinct_frame: "Distinct frame; kept by default.",
    best_quality: "Best quality in this near-duplicate stack.",
    uncertain_similarity: "Similarity is uncertain; inspect before rejecting.",
    quality_tie: "Close quality result; inspect before rejecting.",
    high_confidence_duplicate: "High-confidence near duplicate with a better frame in this stack.",
    decode_error: "Preview could not be analyzed.",
    why: "Why",
    reset: "Reset to suggestion",
    stackTitle: (burst, stack) => `Burst ${burst} · Stack ${stack}`,
    frameCount: count => `${count} ${count === 1 ? "frame" : "frames"}`,
    stackSummary: (count, state, keep, confidence) => `${count} · ${state} · keep ${keep} · confidence ${confidence}`,
    expanded: "expanded",
    collapsed: "collapsed",
    rank: (rank, score) => `Rank ${rank}; quality score ${score}.`,
    sharpness: (whole, subject) => `Whole-frame sharpness ${whole}; subject sharpness ${subject}.`,
    similarity: (distance, confidence) => `Nearest visual distance ${distance}; duplicate confidence ${confidence}.`,
    dimensions: (width, height) => `${width} × ${height}`,
    exifUnavailable: "EXIF unavailable",
    saved: "Review file saved.",
    sourceUnavailable: "This preview is no longer available. Select the folder again.",
    rawPreview: "RAW preview",
  },
  "zh-CN": {
    title: "连拍照片筛选器",
    browserMode: "浏览器模式",
    chooseFolder: "选择照片文件夹",
    localOnly: "照片仅在本机处理",
    saveReview: "保存审核结果",
    cancel: "取消",
    findFilename: "查找文件名",
    filter: "筛选",
    allFrames: "全部照片",
    needsReview: "需要审核",
    kept: "保留",
    rejected: "不保留",
    multiStacks: "多张相似组",
    keep: "保留",
    fit: "适应窗口",
    close: "关闭",
    loading: "正在加载预览",
    images: "照片",
    bursts: "连拍序列",
    stacks: "相似组",
    review: "待审核",
    manual: "手动修改",
    discovering: "正在查找照片",
    loadingEngine: "正在加载本地分析引擎",
    decoding: "正在解码并分析预览图",
    grouping: "正在划分连拍与近似照片",
    rendering: "正在准备审核页面",
    complete: "可以开始审核",
    noImages: "所选文件夹中没有支持的照片。",
    scanCancelled: "扫描已取消。",
    scanFailed: "浏览器扫描未能完成。",
    rawIsolation: "RAW 解码需要跨源隔离。服务工作线程启用后请重新加载页面。",
    rawDecodeFailed: "无法解码 RAW 预览图。",
    previewFailed: "无法解码预览图。",
    unsupportedSkipped: "部分文件无法解码，已跳过。",
    distinct_frame: "这是独特画面，默认保留。",
    best_quality: "这是本相似组中质量最佳的照片。",
    uncertain_similarity: "相似度置信度不足，请检查后再决定。",
    quality_tie: "质量非常接近，请检查后再决定。",
    high_confidence_duplicate: "这是高置信度近似照片，同组中有更好的画面。",
    decode_error: "无法分析预览图。",
    why: "详细原因",
    reset: "恢复建议",
    stackTitle: (burst, stack) => `连拍 ${burst} · 相似组 ${stack}`,
    frameCount: count => `${count} 张照片`,
    stackSummary: (count, state, keep, confidence) => `${count} · ${state} · 保留 ${keep} · 置信度 ${confidence}`,
    expanded: "已展开",
    collapsed: "已折叠",
    rank: (rank, score) => `组内排名 ${rank}；质量分数 ${score}。`,
    sharpness: (whole, subject) => `全图清晰度 ${whole}；主体清晰度 ${subject}。`,
    similarity: (distance, confidence) => `最近视觉距离 ${distance}；重复置信度 ${confidence}。`,
    dimensions: (width, height) => `${width} × ${height}`,
    exifUnavailable: "无 EXIF 信息",
    saved: "审核结果已保存。",
    sourceUnavailable: "预览图已不可用，请重新选择照片文件夹。",
    rawPreview: "RAW 预览图",
  },
};

const elements = Object.fromEntries([
  "folderInput", "pickBtn", "emptyPickBtn", "emptyState", "progressView", "reviewView",
  "stageLabel", "progressDetail", "progressBar", "progressPercent", "cancelBtn", "stats",
  "searchInput", "filterSelect", "stacks", "saveBtn", "sourceLabel", "toast", "viewer",
  "viewerTitle", "viewerKeep", "viewerImage", "viewerLoading", "viewerError", "viewerViewport",
  "zoomOutBtn", "zoomInBtn", "fitBtn", "closeViewerBtn",
].map(id => [id, document.getElementById(id)]));

const queryLocale = new URLSearchParams(location.search).get("lang");
const requestedLocale = Object.hasOwn(messages, queryLocale) ? queryLocale : null;
const defaultLocale = navigator.language.startsWith("zh") ? "zh-CN" : "en";
const storedLocale = localStorage.getItem("burst-locale");
const state = {
  locale: requestedLocale || (Object.hasOwn(messages, storedLocale) ? storedLocale : defaultLocale),
  wasmReady: null,
  result: null,
  assets: new Map(),
  decisions: new Map(),
  expanded: new Set(),
  objectUrls: new Map(),
  scanToken: 0,
  sourceName: "",
  rawDecoder: null,
  viewerAssetId: null,
  viewerPreviousFocus: null,
  viewerScale: 1,
  viewerX: 0,
  viewerY: 0,
  dragging: false,
  dragStart: null,
};

function t(key, ...args) {
  const value = messages[state.locale][key] ?? messages.en[key] ?? key;
  return typeof value === "function" ? value(...args) : value;
}

function applyLocale() {
  document.documentElement.lang = state.locale;
  document.title = t("title");
  document.querySelectorAll("[data-i18n]").forEach(node => {
    node.textContent = t(node.dataset.i18n);
  });
  document.querySelectorAll("[data-i18n-placeholder]").forEach(node => {
    node.placeholder = t(node.dataset.i18nPlaceholder);
  });
  document.querySelectorAll("[data-locale]").forEach(button => {
    button.classList.toggle("active", button.dataset.locale === state.locale);
  });
  elements.sourceLabel.textContent = state.sourceName || t("browserMode");
  elements.filterSelect.setAttribute("aria-label", t("filter"));
  if (state.result) renderReview();
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
  const token = ++state.scanToken;
  resetResult();
  elements.emptyState.hidden = true;
  elements.reviewView.hidden = true;
  elements.progressView.hidden = false;
  elements.saveBtn.hidden = true;
  setProgress("discovering", 0.02);
  await nextPaint();

  const groups = groupSelectedFiles(fileList);
  if (!groups.length) {
    showEmpty(t("noImages"));
    return;
  }
  const firstPath = groups[0].relPath;
  state.sourceName = firstPath.includes("/") ? firstPath.split("/")[0] : t("browserMode");
  elements.sourceLabel.textContent = state.sourceName;

  setProgress("loadingEngine", 0.06);
  try {
    await ensureWasm();
  } catch (error) {
    console.error(error);
    showEmpty(t("scanFailed"));
    return;
  }
  if (token !== state.scanToken) return;

  const session = new BrowserSession();
  const failed = [];
  for (let index = 0; index < groups.length; index += 1) {
    if (token !== state.scanToken) {
      showEmpty(t("scanCancelled"));
      return;
    }
    const group = groups[index];
    const fraction = 0.08 + 0.80 * (index / groups.length);
    setProgress("decoding", fraction, `${index + 1} / ${groups.length} · ${group.relPath}`);
    await nextPaint();
    try {
      const decoded = group.rawOnly
        ? await decodeRaw(group.representative)
        : await decodeBrowserImage(group.representative);
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
      session.add_rgba(input, decoded.width, decoded.height, decoded.rgba);
      state.assets.set(group.id, { ...group, previewUrl: decoded.previewUrl, rawPreview: group.rawOnly });
      state.objectUrls.set(group.id, decoded.previewUrl);
    } catch (error) {
      console.error(group.relPath, error);
      failed.push(group.relPath);
    }
  }

  if (token !== state.scanToken) return;
  if (session.len() === 0) {
    showEmpty(t("noImages"));
    return;
  }
  setProgress("grouping", 0.91);
  await nextPaint();
  try {
    state.result = session.finish(undefined);
  } catch (error) {
    console.error(error);
    showEmpty(t("scanFailed"));
    return;
  }
  setProgress("rendering", 0.97);
  initializeReviewState();
  renderReview();
  setProgress("complete", 1);
  await nextPaint();
  elements.progressView.hidden = true;
  elements.reviewView.hidden = false;
  elements.saveBtn.hidden = false;
  if (failed.length) showToast(`${t("unsupportedSkipped")} (${failed.length})`);
}

async function decodeBrowserImage(file) {
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
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(width * scale));
  canvas.height = Math.max(1, Math.round(height * scale));
  const context = canvas.getContext("2d", { alpha: false });
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  context.drawImage(source, 0, 0, canvas.width, canvas.height);
  return canvas;
}

function canvasBlob(canvas) {
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
    ["manual", state.decisions.size],
  ];
  elements.stats.innerHTML = values.map(([label, value]) => `<div class="stat"><span>${escapeHtml(t(label))}</span><b>${value}</b></div>`).join("");
}

function renderStack(entry) {
  const { stack, allAssets, assets, expanded } = entry;
  const keepCount = allAssets.filter(asset => finalAction(asset) === "keep").length;
  const stateLabel = expanded ? t("expanded") : t("collapsed");
  const count = searchActive() ? `${assets.length} / ${allAssets.length}` : t("frameCount", allAssets.length);
  const diffKeys = metadataDifferences(allAssets);
  return `<section class="stack" data-stack="${stack.id}">
    <div class="stack-header">
      <div><h2>${escapeHtml(t("stackTitle", stack.burst_id, stack.id))}</h2></div>
      <div class="stack-meta">${escapeHtml(t("stackSummary", count, stateLabel, keepCount, Number(stack.similarity_confidence).toFixed(2)))}</div>
      <button type="button" class="stack-toggle" data-toggle-stack="${stack.id}" title="${escapeHtml(stateLabel)}">${expanded ? "−" : "+"}</button>
    </div>
    <div class="frame-grid" ${expanded ? "" : "hidden"}>${assets.map(asset => renderFrame(asset, diffKeys)).join("")}</div>
  </section>`;
}

function renderFrame(asset, diffKeys) {
  const action = finalAction(asset);
  const checked = action === "keep" ? "checked" : "";
  const indeterminate = action === "review" ? "data-indeterminate=\"1\"" : "";
  const source = state.assets.get(asset.id);
  const preview = source?.previewUrl || "";
  const manual = state.decisions.has(asset.id);
  return `<article class="frame ${action}" data-asset="${escapeHtml(asset.id)}">
    <button type="button" class="thumbnail" data-open="${escapeHtml(asset.id)}" aria-label="${escapeHtml(asset.rel_path)}">
      ${preview ? `<img src="${escapeHtml(preview)}" loading="lazy" alt="">` : ""}
      <span class="badge ${action}">${escapeHtml(t(action === "review" ? "review" : action === "reject" ? "rejected" : "keep"))}</span>
    </button>
    <div class="frame-body">
      <label class="keep-control"><input type="checkbox" data-decision="${escapeHtml(asset.id)}" ${checked} ${indeterminate}> ${escapeHtml(t("keep"))}</label>
      <div class="filename">${escapeHtml(asset.rel_path)}</div>
      <div class="exif">${metadataHtml(asset, diffKeys)}</div>
      <div class="reason">${escapeHtml(t(asset.reason_key))}</div>
      <details><summary>${escapeHtml(t("why"))}</summary><ul>
        <li>${escapeHtml(t("rank", asset.rank, Number(asset.score).toFixed(3)))}</li>
        <li>${escapeHtml(t("sharpness", Number(asset.metrics.sharpness).toFixed(1), Number(asset.metrics.subject_sharpness).toFixed(1)))}</li>
        <li>${escapeHtml(t("similarity", Number(asset.similarity.nearest_distance).toFixed(3), Number(asset.similarity.duplicate_confidence).toFixed(2)))}</li>
        <li>${escapeHtml(t("dimensions", asset.source_width, asset.source_height))}</li>
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
  const burst = state.result.bursts.find(item => item.id === asset?.burst_id);
  if (!burst) return;
  const index = burst.asset_ids.indexOf(state.viewerAssetId);
  const next = Math.max(0, Math.min(burst.asset_ids.length - 1, index + delta));
  if (next !== index) openViewer(burst.asset_ids[next], state.viewerPreviousFocus);
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
  const payload = {
    version: 1,
    created_at: new Date().toISOString(),
    source: state.sourceName,
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
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "burst-review.json";
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  showToast(t("saved"));
}

function resetResult() {
  if (!elements.viewer.hidden) closeViewer();
  for (const url of state.objectUrls.values()) URL.revokeObjectURL(url);
  state.objectUrls.clear();
  state.assets.clear();
  state.result = null;
  state.decisions.clear();
  state.expanded.clear();
  elements.searchInput.value = "";
  elements.filterSelect.value = "all";
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

elements.pickBtn.addEventListener("click", () => elements.folderInput.click());
elements.emptyPickBtn.addEventListener("click", () => elements.folderInput.click());
elements.folderInput.addEventListener("change", event => {
  const files = event.target.files;
  if (files?.length) scanFiles(files);
  event.target.value = "";
});
elements.cancelBtn.addEventListener("click", () => {
  state.scanToken += 1;
  resetResult();
  showEmpty(t("scanCancelled"));
});
elements.saveBtn.addEventListener("click", saveReview);
elements.searchInput.addEventListener("input", renderReview);
elements.filterSelect.addEventListener("change", renderReview);
document.querySelectorAll("[data-locale]").forEach(button => {
  button.addEventListener("click", () => {
    state.locale = button.dataset.locale;
    localStorage.setItem("burst-locale", state.locale);
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

applyLocale();
document.documentElement.dataset.crossOriginIsolated = String(crossOriginIsolated);
const testFixture = new URLSearchParams(location.search).get("test-fixture");
if (testFixture === "synthetic" && ["127.0.0.1", "localhost"].includes(location.hostname)) {
  syntheticFixture().then(scanFiles).catch(error => {
    console.error(error);
    showEmpty(t("scanFailed"));
  });
}
