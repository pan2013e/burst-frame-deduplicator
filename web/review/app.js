const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));

const elements = Object.fromEntries([
  "appTitle", "sourceLabel", "searchInput", "filterSelect", "localeMenu", "localeButton",
  "saveButton", "summaryStrip", "clusters", "toast", "viewer", "viewerTitle", "viewerKeep",
  "viewerKeepText", "viewerImage", "viewerLoading", "viewerError", "viewerViewport",
  "previousButton", "nextButton", "zoomOutButton", "zoomInButton", "fitButton",
  "closeViewerButton", "saveDialog", "saveDialogTitle", "saveDialogSubtitle", "closeSaveButton",
  "saveStats", "operationStatus", "destinationLabel", "destinationInput", "refreshScriptsButton",
  "posixTab", "powershellTab", "copyScriptButton", "scriptCode", "exportJsonButton",
  "restoreButton", "moveButton", "confirmDialog", "confirmTitle", "confirmMessage",
  "confirmCancel", "confirmAction",
].map(id => [id, document.getElementById(id)]));

const supportedLocales = ["en", "zh-CN"];
const browserImageExts = new Set(["jpg", "jpeg", "png", "gif", "webp", "bmp"]);
const rawPreviewCache = new Map();
const RAW_CACHE_MAX_BYTES = 192 * 1024 * 1024;
const RAW_CACHE_MAX_ITEMS = 24;

const state = {
  manifest: null,
  review: null,
  moveStatus: { active_asset_ids: [], active_files: 0, active_bytes: 0, destinations: [] },
  decisions: new Map(),
  assets: new Map(),
  clusterAssets: new Map(),
  openedClusters: new Set(),
  closedClusters: new Set(),
  visibleClusterLimit: 80,
  locale: initialLocale(),
  messages: {},
  languageNames: {},
  scripts: null,
  activeScript: detectedScriptKind(),
  scriptRefreshTimer: null,
  pendingOperation: null,
  rawCacheBytes: 0,
  libRawPromise: null,
  viewerAssetId: null,
  viewerClusterIds: [],
  viewerLoadToken: 0,
  viewerUrl: null,
  viewerPreviousFocus: null,
  viewerScale: 1,
  viewerX: 0,
  viewerY: 0,
  dragStart: null,
};

function initialLocale() {
  const requested = new URLSearchParams(location.search).get("lang")
    || localStorage.getItem("burst-locale")
    || navigator.language;
  return String(requested).toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

function detectedScriptKind() {
  const platform = navigator.userAgentData?.platform || navigator.platform || navigator.userAgent;
  return /win/i.test(platform) ? "powershell" : "posix";
}

async function loadLocaleCatalogs() {
  const catalogs = await Promise.all(supportedLocales.map(async code => {
    const response = await fetch(`/locales/${code}.json`);
    if (!response.ok) throw new Error(`locale ${code}: HTTP ${response.status}`);
    return [code, await response.json()];
  }));
  state.messages = Object.fromEntries(catalogs.map(([code, catalog]) => [code, catalog.reviewWeb]));
  state.languageNames = Object.fromEntries(catalogs.map(([code, catalog]) => [code, catalog.languageName]));
}

function t(key, values = {}) {
  const template = state.messages[state.locale]?.[key] ?? state.messages.en?.[key] ?? key;
  return String(template).replace(/\{([a-zA-Z0-9_]+)\}/g, (_, name) => String(values[name] ?? `{${name}}`));
}

function applyLocale() {
  document.documentElement.lang = state.locale;
  document.title = t("title");
  elements.appTitle.textContent = t("title");
  elements.searchInput.placeholder = t("find");
  elements.searchInput.setAttribute("aria-label", t("find"));
  elements.filterSelect.setAttribute("aria-label", t("filter"));
  elements.filterSelect.innerHTML = [
    ["all", "all"], ["review", "needsReview"], ["keep", "kept"],
    ["reject", "rejected"], ["moved", "moved"], ["multi", "multi"],
  ].map(([value, key]) => `<option value="${value}">${escapeHtml(t(key))}</option>`).join("");
  elements.localeButton.setAttribute("aria-label", t("language"));
  $$('[data-locale]').forEach(button => {
    button.textContent = state.languageNames[button.dataset.locale] || button.dataset.locale;
    button.classList.toggle("active", button.dataset.locale === state.locale);
  });
  elements.saveButton.textContent = t("save");
  elements.viewerKeepText.textContent = t("keep");
  setButtonLabel(elements.previousButton, t("previousFrame"));
  setButtonLabel(elements.nextButton, t("nextFrame"));
  setButtonLabel(elements.zoomOutButton, t("zoomOut"));
  setButtonLabel(elements.zoomInButton, t("zoomIn"));
  setButtonLabel(elements.fitButton, t("fit"));
  setButtonLabel(elements.closeViewerButton, t("close"));
  elements.viewerLoading.querySelector("b").textContent = t("loading");
  elements.saveDialogTitle.textContent = t("saveModalTitle");
  elements.saveDialogSubtitle.textContent = t("saveModalSubtitle");
  setButtonLabel(elements.closeSaveButton, t("close"));
  elements.destinationLabel.textContent = t("moveDestinationLabel");
  elements.destinationInput.placeholder = t("runFolderDefault");
  elements.refreshScriptsButton.textContent = t("refreshScript");
  elements.posixTab.textContent = t("macLinuxTab");
  elements.powershellTab.textContent = t("windowsTab");
  elements.copyScriptButton.textContent = t("copyScript");
  elements.exportJsonButton.textContent = t("exportReviewJson");
  elements.restoreButton.textContent = t("restoreMoved");
  elements.confirmCancel.textContent = t("cancel");
  if (state.manifest) {
    const selectedFilter = elements.filterSelect.dataset.selected || "all";
    elements.filterSelect.value = selectedFilter;
    render();
    updateSaveDialog();
  }
}

function setButtonLabel(button, label) {
  button.title = label;
  button.setAttribute("aria-label", label);
}

async function loadRun() {
  const response = await fetch("/api/manifest");
  if (!response.ok) throw new Error(await response.text());
  const data = await response.json();
  state.manifest = data.manifest;
  state.review = data.review;
  state.moveStatus = data.move_status || state.moveStatus;
  state.decisions = new Map(state.review.decisions.map(item => [item.asset_id, item]));
  state.assets = new Map(state.manifest.assets.map(asset => [asset.id, asset]));
  state.clusterAssets.clear();
  for (const cluster of state.manifest.clusters) {
    state.clusterAssets.set(
      cluster.id,
      cluster.asset_ids.map(id => state.assets.get(id)).filter(Boolean)
    );
  }
  render();
}

function render() {
  if (!state.manifest) return;
  elements.sourceLabel.textContent = `${t("source")}: ${sourceFolderName(state.manifest.root)}`;
  renderSummary();
  renderClusters();
}

function sourceFolderName(path) {
  const parts = String(path || "").split(/[\\/]+/).filter(Boolean);
  return parts.at(-1) || t("selectedFolder");
}

function movedAssetIds() {
  return new Set(state.moveStatus.active_asset_ids || []);
}

function finalAction(asset) {
  return state.decisions.get(asset.id)?.decision || asset.suggestion.action;
}

function reviewCounts() {
  const counts = { keep: 0, reject: 0, review: 0, error: 0, moved: movedAssetIds().size };
  for (const asset of state.manifest.assets) {
    const action = finalAction(asset);
    counts[action] = (counts[action] || 0) + 1;
  }
  return counts;
}

function movableRejects() {
  const moved = movedAssetIds();
  return state.manifest.assets.filter(asset => finalAction(asset) === "reject" && !moved.has(asset.id));
}

function renderSummary() {
  const counts = reviewCounts();
  const summary = state.manifest.summary;
  const items = [
    ["images", summary.discovered_assets, ""],
    ["bursts", summary.bursts || state.manifest.bursts?.length || summary.clusters, ""],
    ["stacks", summary.clusters, ""],
    ["keep", counts.keep, "keep"],
    ["reject", counts.reject, "reject"],
    ["review", counts.review, "review"],
    ["moved", counts.moved, "moved"],
  ];
  elements.summaryStrip.innerHTML = items.map(([key, value, className], index) => {
    const separator = index ? '<span class="summary-separator" aria-hidden="true">·</span>' : "";
    return `${separator}<span class="summary-item ${className}"><span>${escapeHtml(t(key))}</span><b>${value}</b></span>`;
  }).join("");
}

function renderClusters() {
  const query = elements.searchInput.value.trim().toLowerCase();
  const filter = elements.filterSelect.value || "all";
  elements.filterSelect.dataset.selected = filter;
  const moved = movedAssetIds();
  const rows = [];
  for (const cluster of state.manifest.clusters) {
    const allAssets = state.clusterAssets.get(cluster.id) || [];
    if (filter === "multi" && allAssets.length <= 1) continue;
    const assets = allAssets.filter(asset => {
      const searchable = `${asset.representative.rel_path} ${asset.stem || ""}`.toLowerCase();
      if (query && !searchable.includes(query)) return false;
      if (filter === "all" || filter === "multi") return true;
      if (filter === "moved") return moved.has(asset.id);
      return finalAction(asset) === filter;
    });
    if (!assets.length) continue;
    rows.push({ cluster, assets, allAssets, collapsed: clusterCollapsed(cluster, allAssets) });
  }
  rows.sort((left, right) => Number(left.collapsed) - Number(right.collapsed) || left.cluster.id - right.cluster.id);
  const visible = rows.slice(0, state.visibleClusterLimit);
  let html = visible.map(row => clusterHtml(row)).join("");
  if (rows.length > visible.length) {
    html += `<button type="button" class="show-more" data-show-more>${escapeHtml(t("showMore", { count: rows.length - visible.length }))}</button>`;
  }
  if (!html) html = `<div class="empty">${escapeHtml(t("noMatches"))}</div>`;
  elements.clusters.innerHTML = html;
  $$('input[data-indeterminate="true"]', elements.clusters).forEach(input => { input.indeterminate = true; });
}

function clusterCollapsed(cluster, assets) {
  if (state.openedClusters.has(cluster.id)) return false;
  if (state.closedClusters.has(cluster.id)) return true;
  return assets.length > 0 && assets.every(asset => finalAction(asset) === "keep");
}

function clusterHtml({ cluster, assets, allAssets, collapsed }) {
  const keepCount = allAssets.filter(asset => finalAction(asset) === "keep").length;
  const diffKeys = exifDiffKeys(allAssets);
  const stateText = t(collapsed ? "collapsed" : "expanded");
  const directory = cluster.directory && cluster.directory !== "."
    ? `<div class="cluster-subtitle">${escapeHtml(cluster.directory)}</div>`
    : "";
  return `<section class="cluster ${collapsed ? "collapsed" : ""}" data-cluster="${cluster.id}">
    <div class="cluster-header">
      <div>
        <h2>${escapeHtml(t("stackTitle", { burst: cluster.burst_id || cluster.id, stack: cluster.id }))}</h2>
        ${directory}
      </div>
      <div class="cluster-meta">${escapeHtml(t("stackSummary", {
        shown: t("shown", { count: assets.length, total: allAssets.length }),
        status: stateText,
        keep: keepCount,
        confidence: Number(cluster.similarity_confidence || 0).toFixed(2),
      }))}</div>
      <button type="button" class="cluster-toggle" aria-expanded="${!collapsed}" title="${escapeHtml(t("collapseTitle"))}">${collapsed ? "+" : "−"}</button>
    </div>
    <div class="frame-grid">${collapsed ? "" : assets.map(asset => frameHtml(asset, diffKeys)).join("")}</div>
  </section>`;
}

function frameHtml(asset, diffKeys) {
  const action = finalAction(asset);
  const moved = movedAssetIds().has(asset.id);
  const displayStatus = moved ? "moved" : action;
  const decision = state.decisions.get(asset.id);
  const checked = action === "keep" ? "checked" : "";
  const indeterminate = action === "review" ? 'data-indeterminate="true"' : "";
  const thumbnail = asset.thumb ? `<img src="/${escapeAttribute(asset.thumb)}" loading="lazy" alt="">` : "";
  return `<article class="frame ${action} ${moved ? "moved" : ""}" data-id="${escapeAttribute(asset.id)}">
    <button type="button" class="thumbnail open-viewer" aria-label="${escapeAttribute(t("openLabel", { filename: asset.representative.rel_path }))}">
      ${thumbnail}<span class="badge ${displayStatus}">${escapeHtml(statusLabel(displayStatus))}</span>
    </button>
    <div class="frame-body">
      <label class="keep-control"><input type="checkbox" ${checked} ${indeterminate}> ${escapeHtml(t("keep"))}</label>
      <div class="filename">${escapeHtml(asset.representative.rel_path)}</div>
      <div class="exif">${exifHtml(asset, diffKeys)}</div>
      <div class="reason">${escapeHtml(shortReason(asset, moved))}</div>
      <details><summary>${escapeHtml(t("why"))}</summary><ul>${detailLines(asset).map(line => `<li>${escapeHtml(line)}</li>`).join("")}</ul></details>
      ${decision?.decision ? `<button type="button" class="reset">${escapeHtml(t("reset"))}</button>` : ""}
    </div>
  </article>`;
}

function statusLabel(status) {
  if (status === "keep") return t("keep");
  if (status === "reject") return t("reject");
  if (status === "moved") return t("moved");
  if (status === "error") return t("error");
  return t("review");
}

function shortReason(asset, moved) {
  if (moved) return t("movedReason");
  if (asset.suggestion.action === "keep") {
    return asset.suggestion.reason === "distinct frame" ? t("distinct") : t("best");
  }
  if (asset.suggestion.action === "review") return t("closeCall");
  if (asset.suggestion.action === "error") return asset.error || t("decodeFailed");
  return t("duplicate");
}

function detailLines(asset) {
  const metrics = asset.metrics || {};
  const similarity = asset.similarity || {};
  const detector = asset.detector
    ? `${asset.detector.backend}: ${asset.detector.explanation || ""}`
    : t("detectorOff");
  return [
    t("rank", { rank: asset.suggestion.rank, score: number(asset.suggestion.score, 3) }),
    t("sharpness", {
      whole: number(metrics.sharpness, 1),
      subject: number(metrics.subject_sharpness, 1),
      completeness: number(metrics.completeness, 2),
    }),
    t("similarity", {
      distance: number(similarity.nearest_distance, 3),
      subject: number(similarity.nearest_subject_distance, 3),
      scene: number(similarity.nearest_global_distance, 3),
      confidence: number(similarity.duplicate_confidence, 2),
    }),
    t("backend", { backend: asset.feature_backend || t("unknown"), decoder: asset.decoder || t("unknown") }),
    detector,
    t("exposure", { score: number(metrics.exposure_score, 2), clipped: number((metrics.clipped_fraction || 0) * 100, 2) }),
  ];
}

const exifFields = ["iso", "aperture", "shutter", "focal_length_mm", "focal_length_35mm"];

function exifDiffKeys(assets) {
  const result = new Set();
  for (const key of exifFields) {
    const values = new Set(assets.map(asset => metadataCompareValue(asset.metadata, key)).filter(value => value !== null));
    if (values.size > 1) result.add(key);
  }
  return result;
}

function exifHtml(asset, diffKeys) {
  const values = exifFields.map(key => {
    const text = metadataDisplayValue(asset.metadata || {}, key);
    return text ? `<span class="chip ${diffKeys.has(key) ? "diff" : ""}">${escapeHtml(text)}</span>` : "";
  }).filter(Boolean);
  return values.length ? values.join("") : `<span class="chip">${escapeHtml(t("exifUnavailable"))}</span>`;
}

function metadataCompareValue(metadata = {}, key) {
  const value = metadata[key];
  if (value === null || value === undefined || value === "") return null;
  return typeof value === "number" ? number(value, key === "aperture" || key === "focal_length_mm" ? 1 : 0) : String(value);
}

function metadataDisplayValue(metadata, key) {
  const value = metadata[key];
  if (value === null || value === undefined || value === "") return "";
  if (key === "iso") return `ISO ${value}`;
  if (key === "aperture") return `f/${number(value, 1)}`;
  if (key === "shutter") return String(value);
  if (key === "focal_length_mm") return `${number(value, 1)} mm`;
  if (key === "focal_length_35mm") return `${value} mm eq`;
  return String(value);
}

function number(value, digits) {
  return Number(value || 0).toFixed(digits).replace(/\.0+$/, "");
}

async function saveDecision(assetId, decision) {
  const response = await fetch("/api/decision", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ asset_id: assetId, decision, note: null }),
  });
  if (!response.ok) throw new Error(await response.text());
  state.review = await response.json();
  state.decisions = new Map(state.review.decisions.map(item => [item.asset_id, item]));
  render();
  updateSaveDialog();
}

async function openSaveDialog() {
  elements.saveButton.disabled = true;
  try {
    const response = await fetch("/api/export", { method: "POST" });
    if (!response.ok) throw new Error(await response.text());
    await loadRun();
    clearOperationStatus();
    updateSaveDialog();
    if (!elements.saveDialog.open) elements.saveDialog.showModal();
    await refreshScripts();
  } catch (error) {
    showToast(error.message);
  } finally {
    elements.saveButton.disabled = false;
  }
}

function updateSaveDialog() {
  if (!state.manifest) return;
  const counts = reviewCounts();
  elements.saveStats.innerHTML = [
    ["keep", counts.keep], ["reject", counts.reject], ["review", counts.review], ["moved", counts.moved],
  ].map(([key, value]) => `<div class="save-stat"><span>${escapeHtml(t(key))}</span><b>${value}</b></div>`).join("");
  const rejectCount = movableRejects().length;
  elements.moveButton.textContent = rejectCount ? t("moveRejects", { count: rejectCount }) : t("noRejects");
  elements.moveButton.disabled = rejectCount === 0;
  elements.restoreButton.hidden = counts.moved === 0;
}

async function refreshScripts() {
  elements.refreshScriptsButton.disabled = true;
  try {
    const destination = elements.destinationInput.value.trim() || null;
    const response = await fetch("/api/move-scripts", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ destination }),
    });
    if (!response.ok) throw new Error(await response.text());
    state.scripts = await response.json();
    renderScript();
  } catch (error) {
    setOperationStatus(error.message, true);
  } finally {
    elements.refreshScriptsButton.disabled = false;
  }
}

function renderScript() {
  elements.posixTab.setAttribute("aria-selected", String(state.activeScript === "posix"));
  elements.powershellTab.setAttribute("aria-selected", String(state.activeScript === "powershell"));
  elements.scriptCode.textContent = state.scripts?.[state.activeScript] || "";
}

function exportReviewJson() {
  const payload = {
    version: 1,
    source: state.manifest.root,
    run_created_at: state.review.run_created_at,
    updated_at: state.review.updated_at,
    decisions: state.review.decisions,
    move_status: state.moveStatus,
    final_counts: reviewCounts(),
  };
  const blob = new Blob([`${JSON.stringify(payload, null, 2)}\n`], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = "burst-review.json";
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function requestConfirmation(operation) {
  state.pendingOperation = operation;
  const moving = operation === "move";
  const count = moving ? movableRejects().length : movedAssetIds().size;
  elements.confirmTitle.textContent = t(moving ? "moveConfirmTitle" : "restoreConfirmTitle");
  elements.confirmMessage.textContent = t(moving ? "moveConfirm" : "restoreConfirm", { count });
  elements.confirmAction.textContent = t(moving ? "move" : "restore");
  elements.confirmAction.className = moving ? "danger" : "primary";
  elements.confirmDialog.returnValue = "";
  elements.confirmDialog.showModal();
}

async function runFileOperation(operation) {
  const moving = operation === "move";
  elements.moveButton.disabled = true;
  elements.restoreButton.disabled = true;
  try {
    const endpoint = moving ? "/api/move-rejects" : "/api/restore-rejects";
    const body = moving
      ? { confirm: true, destination: elements.destinationInput.value.trim() || null }
      : { confirm: true, asset_ids: null };
    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!response.ok) throw new Error(await response.text());
    const result = await response.json();
    await loadRun();
    updateSaveDialog();
    await refreshScripts();
    if (!result.source_available) {
      setOperationStatus(t(moving ? "moveSourceUnavailable" : "restoreSourceUnavailable"), true);
    } else if (moving) {
      setOperationStatus(t("movedResult", {
        files: result.moved_files,
        assets: result.moved_assets,
        destination: result.destination,
        missing: result.missing_files.length,
        failed: result.failed_files.length,
      }), result.failed_files.length > 0);
    } else {
      setOperationStatus(t("restored", {
        files: result.restored_files,
        assets: result.restored_assets,
        missing: result.missing_files.length,
        failed: result.failed_files.length,
      }), result.failed_files.length > 0);
    }
  } catch (error) {
    setOperationStatus(error.message, true);
  } finally {
    updateSaveDialog();
    elements.restoreButton.disabled = false;
  }
}

function setOperationStatus(message, isError = false) {
  elements.operationStatus.textContent = message;
  elements.operationStatus.hidden = false;
  elements.operationStatus.classList.toggle("error", isError);
}

function clearOperationStatus() {
  elements.operationStatus.hidden = true;
  elements.operationStatus.textContent = "";
  elements.operationStatus.classList.remove("error");
}

async function openViewer(asset) {
  const loadToken = ++state.viewerLoadToken;
  state.viewerAssetId = asset.id;
  const cluster = state.manifest.clusters.find(item => item.id === asset.cluster_id);
  state.viewerClusterIds = cluster?.asset_ids || [asset.id];
  state.viewerPreviousFocus ||= document.activeElement;
  elements.viewerTitle.textContent = asset.representative.rel_path;
  elements.viewer.hidden = false;
  elements.viewer.setAttribute("aria-hidden", "false");
  elements.viewer.focus({ preventScroll: true });
  elements.viewerImage.hidden = true;
  elements.viewerError.hidden = true;
  elements.viewerLoading.hidden = false;
  syncViewerDecision(asset);
  updateViewerNavigation();
  revokeViewerUrl();
  try {
    const blob = rawOnly(asset) ? await rawPreviewBlob(asset) : await sourceImageBlob(asset);
    if (loadToken !== state.viewerLoadToken) return;
    showViewerBlob(blob, loadToken);
  } catch (error) {
    if (loadToken !== state.viewerLoadToken) return;
    elements.viewerError.textContent = friendlyPreviewError(error.message);
    elements.viewerError.hidden = false;
    elements.viewerLoading.hidden = true;
  }
}

function rawOnly(asset) {
  const files = asset.files || [];
  return files.some(file => file.kind === "raw")
    && !files.some(file => browserImageExts.has(String(file.extension || "").toLowerCase()));
}

async function sourceImageBlob(asset) {
  const response = await fetch(`/api/image/${encodeURIComponent(asset.id)}`);
  if (!response.ok) throw new Error(await response.text());
  return response.blob();
}

async function rawPreviewBlob(asset) {
  const cached = getRawCache(asset.id);
  if (cached) return cached;
  try {
    const blob = await decodeRawWithWasm(asset);
    putRawCache(asset.id, blob);
    return blob;
  } catch (wasmError) {
    const response = await fetch(`/api/preview/${encodeURIComponent(asset.id)}`);
    if (!response.ok) throw new Error(await response.text());
    const blob = await response.blob();
    putRawCache(asset.id, blob);
    return blob;
  }
}

async function decodeRawWithWasm(asset) {
  if (!state.libRawPromise) {
    state.libRawPromise = import("/vendor/libraw-wasm/index.js").then(module => module.default);
  }
  const LibRaw = await state.libRawPromise;
  const response = await fetch(`/api/source/${encodeURIComponent(asset.id)}`);
  if (!response.ok) throw new Error(await response.text());
  const bytes = new Uint8Array(await response.arrayBuffer());
  const raw = new LibRaw();
  try {
    await raw.open(bytes, { halfSize: true, useCameraWb: true, outputColor: 1, outputBps: 8, userQual: 1 });
    const imageData = await raw.imageData();
    return rawImageDataToBlob(imageData);
  } finally {
    if (typeof raw.dispose === "function") raw.dispose();
  }
}

async function rawImageDataToBlob(imageData) {
  const { width, height, data } = imageData;
  if (!width || !height || !data || data.length < width * height * 3) {
    throw new Error(t("rawDecodeFailed"));
  }
  const rgba = new Uint8ClampedArray(width * height * 4);
  for (let source = 0, target = 0; source < width * height * 3; source += 3, target += 4) {
    rgba[target] = data[source];
    rgba[target + 1] = data[source + 1];
    rgba[target + 2] = data[source + 2];
    rgba[target + 3] = 255;
  }
  const canvas = new OffscreenCanvas(width, height);
  const context = canvas.getContext("2d", { alpha: false });
  context.putImageData(new ImageData(rgba, width, height), 0, 0);
  return canvas.convertToBlob({ type: "image/jpeg", quality: 0.92 });
}

function getRawCache(assetId) {
  const entry = rawPreviewCache.get(assetId);
  if (!entry) return null;
  rawPreviewCache.delete(assetId);
  rawPreviewCache.set(assetId, entry);
  return entry.blob;
}

function putRawCache(assetId, blob) {
  if (!blob?.size || blob.size > RAW_CACHE_MAX_BYTES) return;
  const existing = rawPreviewCache.get(assetId);
  if (existing) state.rawCacheBytes -= existing.size;
  rawPreviewCache.delete(assetId);
  rawPreviewCache.set(assetId, { blob, size: blob.size });
  state.rawCacheBytes += blob.size;
  while (rawPreviewCache.size > RAW_CACHE_MAX_ITEMS || state.rawCacheBytes > RAW_CACHE_MAX_BYTES) {
    const oldest = rawPreviewCache.keys().next().value;
    const entry = rawPreviewCache.get(oldest);
    state.rawCacheBytes -= entry?.size || 0;
    rawPreviewCache.delete(oldest);
  }
}

function showViewerBlob(blob, loadToken) {
  state.viewerUrl = URL.createObjectURL(blob);
  elements.viewerImage.onload = () => {
    if (loadToken !== state.viewerLoadToken) return;
    fitViewer();
    elements.viewerLoading.hidden = true;
  };
  elements.viewerImage.onerror = () => {
    elements.viewerError.textContent = t("previewUnavailable");
    elements.viewerError.hidden = false;
    elements.viewerLoading.hidden = true;
  };
  elements.viewerImage.src = state.viewerUrl;
  elements.viewerImage.hidden = false;
}

function syncViewerDecision(asset) {
  const action = finalAction(asset);
  elements.viewerKeep.checked = action === "keep";
  elements.viewerKeep.indeterminate = action === "review";
}

function updateViewerNavigation() {
  const index = state.viewerClusterIds.indexOf(state.viewerAssetId);
  elements.previousButton.disabled = index <= 0;
  elements.nextButton.disabled = index < 0 || index >= state.viewerClusterIds.length - 1;
}

function showAdjacentViewer(delta) {
  const index = state.viewerClusterIds.indexOf(state.viewerAssetId);
  const next = index + delta;
  if (next < 0 || next >= state.viewerClusterIds.length) return;
  const asset = state.assets.get(state.viewerClusterIds[next]);
  if (asset) openViewer(asset);
}

function closeViewer() {
  state.viewerLoadToken += 1;
  elements.viewer.hidden = true;
  elements.viewer.setAttribute("aria-hidden", "true");
  elements.viewerImage.removeAttribute("src");
  revokeViewerUrl();
  state.viewerAssetId = null;
  state.viewerClusterIds = [];
  if (state.viewerPreviousFocus?.isConnected) state.viewerPreviousFocus.focus({ preventScroll: true });
  state.viewerPreviousFocus = null;
}

function revokeViewerUrl() {
  if (state.viewerUrl) URL.revokeObjectURL(state.viewerUrl);
  state.viewerUrl = null;
}

function fitViewer() {
  if (!elements.viewerImage.naturalWidth || !elements.viewerImage.naturalHeight) return;
  state.viewerScale = Math.min(
    elements.viewerViewport.clientWidth / elements.viewerImage.naturalWidth,
    elements.viewerViewport.clientHeight / elements.viewerImage.naturalHeight,
    1
  );
  state.viewerX = 0;
  state.viewerY = 0;
  applyViewerTransform();
}

function zoomViewer(factor) {
  state.viewerScale = Math.max(0.04, Math.min(10, state.viewerScale * factor));
  applyViewerTransform();
}

function applyViewerTransform() {
  elements.viewerImage.style.transform = `translate(calc(-50% + ${state.viewerX}px), calc(-50% + ${state.viewerY}px)) scale(${state.viewerScale})`;
}

function friendlyPreviewError(message) {
  if (/not accessible|unavailable|not found/i.test(message)) return t("previewSourceUnavailable");
  return `${t("previewUnavailable")}\n${message}`;
}

function showToast(message) {
  elements.toast.textContent = message;
  elements.toast.hidden = false;
  clearTimeout(showToast.timer);
  showToast.timer = setTimeout(() => { elements.toast.hidden = true; }, 4400);
}

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, character => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[character]));
}

function escapeAttribute(value) {
  return escapeHtml(value).replace(/`/g, "&#96;");
}

elements.clusters.addEventListener("change", async event => {
  const input = event.target.closest('.frame input[type="checkbox"]');
  if (!input) return;
  input.indeterminate = false;
  try {
    await saveDecision(input.closest(".frame").dataset.id, input.checked ? "keep" : "reject");
  } catch (error) {
    showToast(error.message);
  }
});

elements.clusters.addEventListener("click", async event => {
  const showMore = event.target.closest("[data-show-more]");
  if (showMore) {
    state.visibleClusterLimit += 80;
    renderClusters();
    return;
  }
  const toggle = event.target.closest(".cluster-toggle");
  if (toggle) {
    const cluster = toggle.closest(".cluster");
    const id = Number(cluster.dataset.cluster);
    if (cluster.classList.contains("collapsed")) {
      state.openedClusters.add(id);
      state.closedClusters.delete(id);
    } else {
      state.closedClusters.add(id);
      state.openedClusters.delete(id);
    }
    renderClusters();
    return;
  }
  const reset = event.target.closest(".reset");
  if (reset) {
    try { await saveDecision(reset.closest(".frame").dataset.id, null); }
    catch (error) { showToast(error.message); }
    return;
  }
  const thumbnail = event.target.closest(".open-viewer");
  if (thumbnail) {
    const asset = state.assets.get(thumbnail.closest(".frame").dataset.id);
    if (asset) openViewer(asset);
  }
});

elements.searchInput.addEventListener("input", () => {
  clearTimeout(elements.searchInput.renderTimer);
  elements.searchInput.renderTimer = setTimeout(() => {
    state.visibleClusterLimit = 80;
    renderClusters();
  }, 90);
});

elements.filterSelect.addEventListener("change", () => {
  state.visibleClusterLimit = 80;
  elements.filterSelect.dataset.selected = elements.filterSelect.value;
  renderClusters();
});

$$('[data-locale]').forEach(button => button.addEventListener("click", () => {
  state.locale = button.dataset.locale;
  localStorage.setItem("burst-locale", state.locale);
  elements.localeMenu.open = false;
  applyLocale();
}));

elements.saveButton.addEventListener("click", openSaveDialog);
elements.refreshScriptsButton.addEventListener("click", refreshScripts);
elements.destinationInput.addEventListener("input", () => {
  clearTimeout(state.scriptRefreshTimer);
  state.scriptRefreshTimer = setTimeout(refreshScripts, 450);
});
elements.posixTab.addEventListener("click", () => { state.activeScript = "posix"; renderScript(); });
elements.powershellTab.addEventListener("click", () => { state.activeScript = "powershell"; renderScript(); });
elements.copyScriptButton.addEventListener("click", async () => {
  await navigator.clipboard.writeText(elements.scriptCode.textContent);
  showToast(t("scriptCopied"));
});
elements.exportJsonButton.addEventListener("click", exportReviewJson);
elements.moveButton.addEventListener("click", () => requestConfirmation("move"));
elements.restoreButton.addEventListener("click", () => requestConfirmation("restore"));
elements.confirmDialog.addEventListener("close", () => {
  if (elements.confirmDialog.returnValue === "confirm" && state.pendingOperation) {
    runFileOperation(state.pendingOperation);
  }
  state.pendingOperation = null;
});

elements.viewerKeep.addEventListener("change", async event => {
  if (!state.viewerAssetId) return;
  event.target.indeterminate = false;
  try {
    await saveDecision(state.viewerAssetId, event.target.checked ? "keep" : "reject");
    const asset = state.assets.get(state.viewerAssetId);
    if (asset) syncViewerDecision(asset);
  } catch (error) {
    showToast(error.message);
  }
});
elements.previousButton.addEventListener("click", () => showAdjacentViewer(-1));
elements.nextButton.addEventListener("click", () => showAdjacentViewer(1));
elements.zoomOutButton.addEventListener("click", () => zoomViewer(0.8));
elements.zoomInButton.addEventListener("click", () => zoomViewer(1.25));
elements.fitButton.addEventListener("click", fitViewer);
elements.closeViewerButton.addEventListener("click", closeViewer);
elements.viewerViewport.addEventListener("wheel", event => {
  event.preventDefault();
  zoomViewer(event.deltaY < 0 ? 1.12 : 0.89);
}, { passive: false });
elements.viewerViewport.addEventListener("pointerdown", event => {
  state.dragStart = { x: event.clientX, y: event.clientY, viewerX: state.viewerX, viewerY: state.viewerY };
  elements.viewerViewport.classList.add("dragging");
  elements.viewerViewport.setPointerCapture(event.pointerId);
});
elements.viewerViewport.addEventListener("pointermove", event => {
  if (!state.dragStart) return;
  state.viewerX = state.dragStart.viewerX + event.clientX - state.dragStart.x;
  state.viewerY = state.dragStart.viewerY + event.clientY - state.dragStart.y;
  applyViewerTransform();
});
elements.viewerViewport.addEventListener("pointerup", event => {
  state.dragStart = null;
  elements.viewerViewport.classList.remove("dragging");
  elements.viewerViewport.releasePointerCapture(event.pointerId);
});

document.addEventListener("keydown", event => {
  if (elements.viewer.hidden) return;
  if (event.key === "Escape") closeViewer();
  if (event.key === "ArrowLeft") { event.preventDefault(); showAdjacentViewer(-1); }
  if (event.key === "ArrowRight") { event.preventDefault(); showAdjacentViewer(1); }
});

async function initialize() {
  await loadLocaleCatalogs();
  applyLocale();
  await loadRun();
}

initialize().catch(error => {
  elements.clusters.innerHTML = `<div class="empty">${escapeHtml(error.message)}</div>`;
});
