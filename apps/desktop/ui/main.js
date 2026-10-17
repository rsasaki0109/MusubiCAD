const { invoke } = window.__TAURI__.core;
const { open, save } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;

const preview = document.getElementById("preview");
const previewFrame = document.getElementById("preview-frame");
const previewSyncBadge = document.getElementById("preview-sync-badge");
const highlightOverlay = document.getElementById("highlight-overlay");
const status = document.getElementById("status");
const docInfo = document.getElementById("doc-info");
const previewInfo = document.getElementById("preview-info");
const selectionInfo = document.getElementById("selection-info");
const regenerationInfo = document.getElementById("regeneration-info");
const intentInfo = document.getElementById("intent-info");
const intentHint = document.getElementById("intent-hint");
const parametersPanel = document.getElementById("parameters");
const templateSelect = document.getElementById("template-select");
const openBtn = document.getElementById("open-btn");
const refreshBtn = document.getElementById("refresh-btn");
const undoBtn = document.getElementById("undo-btn");
const redoBtn = document.getElementById("redo-btn");
const viewportBtn = document.getElementById("viewport-btn");
const createBtn = document.getElementById("create-btn");

let documentLoadSequence = 0;
let previewSelectionFrame = null;
let selectedBinding = null;
let selectedFrame = null;
let featureTree = null;
let reviewedFeatureEdit = null;
let featureBusy = false;
let documentMutationBusy = false;
let featureReviewSequence = 0;
let currentPath = null;
let parameterRows = [];
// The backend owns complete document snapshots. The UI treats this value as
// opaque and only consumes the capability flags returned beside it.
let documentHistoryState = null;

const PREVIEW_WIDTH = 960;
const PREVIEW_HEIGHT = 540;

function setStatus(message) {
  status.textContent = message;
}

function renderInfo(container, entries) {
  container.replaceChildren();
  for (const [label, value] of entries) {
    const dt = document.createElement("dt");
    dt.textContent = label;
    const dd = document.createElement("dd");
    dd.textContent = String(value);
    container.append(dt, dd);
  }
}

function formatVec3(values) {
  return values.map((v) => v.toFixed(4)).join(", ");
}

// One item per line; the panel's `dd` keeps the line breaks.
function formatList(items) {
  return items && items.length ? items.join("\n") : "—";
}

// Where the document stops regenerating, from the same read-only backend
// query as `musubicad intent <path> regen`.
function renderRegeneration(result) {
  const failure = result.failure;
  const entries = [["Kernel", result.kernel]];
  if (failure) {
    entries.push(
      ["Status", `failed at ${failure.stage.replaceAll("_", " ")}`],
      ["Failing node", failure.node ?? "—"],
      ["Error", failure.error],
      ["Blocked", formatList(failure.blocked_features)],
      ["Not reached", formatList(failure.not_reached_features)],
      ["Completed", `${failure.completed_features.length} features`],
    );
  } else {
    entries.push(["Status", "regenerated"]);
  }
  if (result.body_feature) {
    entries.push([failure ? "Upstream body" : "Body", result.body_feature]);
  }
  if (result.mass_kg != null) {
    entries.push(["Mass (kg)", result.mass_kg.toFixed(4)]);
  }
  renderInfo(regenerationInfo, entries);
  regenerationInfo.classList.toggle("failed", Boolean(failure));
}

function renderRegenerationUnavailable(error) {
  renderInfo(regenerationInfo, [["Status", `unavailable: ${error}`]]);
  regenerationInfo.classList.remove("failed");
}

function renderParameterIntent(item) {
  const parameter = item.parameter;
  const value = parameter.value_m == null ? "" : ` = ${parameter.value_m} m`;
  intentHint.hidden = true;
  renderInfo(intentInfo, [
    ["Parameter", `${parameter.name} (${parameter.expr}${value})`],
    ["Driven by", formatList(item.driven_by)],
    ["Drives", formatList(item.drives_parameters)],
    ["Sketches", formatList(item.sketches)],
    ["Directly affects", formatList(item.directly_affected_features)],
    ["Regenerates", formatList(item.predicted_dirty_features)],
    ["Assertions", formatList(item.assertions)],
  ]);
}

function renderReferenceIntent(item) {
  const reference = item.reference;
  intentHint.hidden = true;
  renderInfo(intentInfo, [
    ["Reference", reference.ref_id],
    ["Created by", `${reference.created_by} (${reference.role ?? "—"})`],
    ["Features", formatList(item.consuming_features)],
    ["Mates", formatList(item.consuming_mates)],
    ["Regenerates", formatList(item.predicted_dirty_features)],
    ["Assertions", formatList(item.assertions)],
  ]);
}

function clearIntent() {
  intentHint.hidden = false;
  renderInfo(intentInfo, []);
}

async function showParameterIntent(row) {
  const item = await invoke("inspect_parameter_intent_cmd", {
    path: currentPath,
    id: row.id,
  });
  renderParameterIntent(item);
  setStatus(`Intent: ${row.name}`);
}

async function showReferenceIntent(refId) {
  const item = await invoke("inspect_reference_intent_cmd", {
    path: currentPath,
    reference: refId,
  });
  renderReferenceIntent(item);
}

function previewImageCoords(event) {
  const rect = preview.getBoundingClientRect();
  const naturalW = preview.naturalWidth || PREVIEW_WIDTH;
  const naturalH = preview.naturalHeight || PREVIEW_HEIGHT;
  const scale = Math.min(rect.width / naturalW, rect.height / naturalH);
  const renderW = naturalW * scale;
  const renderH = naturalH * scale;
  const offsetX = (rect.width - renderW) / 2;
  const offsetY = (rect.height - renderH) / 2;
  const x = ((event.clientX - rect.left - offsetX) / renderW) * naturalW;
  const y = ((event.clientY - rect.top - offsetY) / renderH) * naturalH;
  if (x < 0 || y < 0 || x > naturalW || y > naturalH) {
    return null;
  }
  return { x, y };
}

function filterExistingParameterIds(ids) {
  const available = new Set(parameterRows.map((row) => row.id));
  return ids.filter((id) => available.has(id));
}

function relatedParameterCandidates(selection) {
  if (selection.kind === "none") {
    return [];
  }
  if (selection.kind === "sketch_line") {
    if (selection.sketch_id === "sketch:profile") {
      return [
        "param:inner_radius",
        "param:outer_radius",
        "param:height",
        "param:revolve_angle",
      ];
    }
    return ["param:width", "param:height"];
  }

  const feature = selection.inferred_feature_id ?? "";
  const role = selection.face_role ?? "";

  if (feature.includes("revolve")) {
    return [
      "param:revolve_angle",
      "param:outer_radius",
      "param:inner_radius",
      "param:height",
    ];
  }
  if (feature.includes("hole") || role === "cylindrical") {
    return ["param:hole_diameter", "param:hole_pitch", "param:thickness"];
  }
  if (feature.includes("boss")) {
    return ["param:hole_diameter", "param:boss_height", "param:thickness"];
  }
  if (feature.includes("pattern") || feature.includes("pin_row") || feature.includes("hole_row")) {
    return ["param:hole_pitch", "param:hole_diameter"];
  }
  if (feature.includes("mirror")) {
    return ["param:hole_pitch", "param:width"];
  }
  if (feature.includes("fillet")) {
    return ["param:fillet_radius"];
  }
  if (feature.includes("chamfer")) {
    return ["param:chamfer_distance"];
  }
  if (feature.includes("extrude")) {
    if (role === "top" || role === "bottom") {
      return ["param:thickness"];
    }
    if (role === "+x" || role === "-x") {
      return ["param:width"];
    }
    if (role === "+y" || role === "-y") {
      return ["param:height"];
    }
    return ["param:width", "param:height", "param:thickness"];
  }
  return [];
}

function relatedParameterIds(selection, summary) {
  if (summary?.related_parameter_ids?.length) {
    return summary.related_parameter_ids;
  }
  return filterExistingParameterIds(relatedParameterCandidates(selection));
}

let paramFocusTimer = null;

function clearParameterFocus() {
  for (const row of parametersPanel.querySelectorAll(".param-row.related")) {
    row.classList.remove("related");
  }
  if (paramFocusTimer) {
    clearTimeout(paramFocusTimer);
    paramFocusTimer = null;
  }
}

function focusRelatedParameters(ids) {
  clearParameterFocus();
  if (!ids.length) {
    return [];
  }

  const names = [];
  let firstInput = null;
  for (const id of ids) {
    const input = document.getElementById(`param-${id}`);
    if (!input) {
      continue;
    }
    const row = input.closest(".param-row");
    if (row) {
      row.classList.add("related");
    }
    const rowData = parameterRows.find((entry) => entry.id === id);
    if (rowData) {
      names.push(rowData.name);
    }
    if (!firstInput) {
      firstInput = input;
    }
  }

  if (firstInput) {
    firstInput.scrollIntoView({ block: "nearest", behavior: "smooth" });
    firstInput.focus({ preventScroll: true });
  }

  paramFocusTimer = setTimeout(clearParameterFocus, 8000);
  return names;
}

function renderSelection(summary) {
  const entries = [
    ["Pixel", `${summary.x.toFixed(1)}, ${summary.y.toFixed(1)}`],
    ["Kind", summary.selection.kind ?? "none"],
  ];

  if (summary.selection.kind === "sketch_line") {
    const line = summary.selection;
    entries.push(
      ["Line index", line.line_index],
      ["Sketch", line.sketch_id ?? "—"],
      ["Entity", line.entity_id ?? "—"],
      ["Entity kind", line.entity_kind ?? "—"],
      ["Construction", line.construction],
      ["Start (m)", formatVec3(line.start_m)],
      ["End (m)", formatVec3(line.end_m)],
    );
  } else if (summary.selection.kind === "solid_triangle") {
    const solid = summary.selection;
    entries.push(
      ["Triangle", solid.triangle_index],
      ["Face group", solid.face_group_index ?? "—"],
      ["Face role", solid.face_role ?? "—"],
      ["Kernel face", solid.kernel_face_id ?? "—"],
      ["Feature", solid.inferred_feature_id ?? "—"],
      ["Topo ref", solid.inferred_topo_ref_id ?? "—"],
    );
    if (solid.face_centroid_m) {
      entries.push(["Centroid (m)", formatVec3(solid.face_centroid_m)]);
    }
    if (solid.face_normal_m) {
      entries.push(["Normal (m)", formatVec3(solid.face_normal_m)]);
    }
  }

  const relatedNames = relatedParameterIds(summary.selection, summary)
    .map((id) => parameterRows.find((row) => row.id === id)?.name)
    .filter(Boolean);
  if (relatedNames.length) {
    entries.push(["Related params", relatedNames.join(", ")]);
  }

  renderInfo(selectionInfo, entries);
}

function clearSelection() {
  selectedBinding = null;
  selectedFrame = null;
  document.getElementById("selection-edit").hidden = true;
  renderInfo(selectionInfo, [["Kind", "none"]]);
  renderHighlight([]);
  clearParameterFocus();
}

function renderHighlight(segments) {
  highlightOverlay.replaceChildren();
  for (const segment of segments) {
    const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
    line.setAttribute("x1", String(segment.start_px[0]));
    line.setAttribute("y1", String(segment.start_px[1]));
    line.setAttribute("x2", String(segment.end_px[0]));
    line.setAttribute("y2", String(segment.end_px[1]));
    highlightOverlay.append(line);
  }
}

function handlePickSummary(summary, sourceLabel) {
  cancelFeatureReview();
  selectedBinding = null;
  selectedFrame = null;
  const frame = summary.selection_frame;
  const matchesPreview = sourceLabel !== "preview" || (frame?.source_revision === previewSelectionFrame?.source_revision && frame?.scene_revision === previewSelectionFrame?.scene_revision);
  const currentSource = frame?.source_revision === featureTree?.revision;
  if (summary.selection_binding && matchesPreview && currentSource) {
    selectedBinding = summary.selection_binding;
    selectedFrame = frame;
  }
  renderSelectedDimensionForm();
  renderSelection(summary);
  renderHighlight(summary.highlight_segments_px ?? []);
  if (summary.selection.kind === "none") {
    setStatus(
      sourceLabel === "preview"
        ? "No geometry at click point."
        : "No geometry picked in 3D viewport.",
    );
    return;
  }
  const refId = summary.selection.inferred_topo_ref_id;
  if (refId && currentPath) {
    showReferenceIntent(refId).catch((error) => setStatus(`Error: ${error}`));
  }
  const label = summary.selection.kind.replaceAll("_", " ");
  // Related parameter/name guesses remain display hints; selecting geometry
  // does not focus an editable parameter or create an inferred mutation.
  const relatedNames = [];
  let message =
    sourceLabel === "preview" ? `Selected ${label}` : `3D viewport: ${label}`;
  if (relatedNames.length) {
    message += ` — related: ${relatedNames.join(", ")}`;
  }
  if (!selectedBinding) message += " — no current unique editable source binding; refresh or select supported geometry.";
  setStatus(message);
}

async function pickAtPreview(event) {
  if (!currentPath || documentMutationBusy) {
    return;
  }
  const path = currentPath;
  const sequence = documentLoadSequence;
  const coords = previewImageCoords(event);
  if (!coords) {
    return;
  }

  setStatus(`Picking at ${coords.x.toFixed(0)}, ${coords.y.toFixed(0)}…`);
  try {
    const summary = await invoke("pick_document_cmd", {
      path,
      x: coords.x,
      y: coords.y,
    });
    if (sequence !== documentLoadSequence || currentPath !== path || documentMutationBusy) return;
    handlePickSummary(summary, "preview");
  } catch (error) {
    setStatus(`Error: ${error}`);
  }
}

let previewSyncTimer = null;

function clearPreviewSyncing() {
  if (previewSyncTimer) {
    clearTimeout(previewSyncTimer);
    previewSyncTimer = null;
  }
  previewFrame.classList.remove("camera-syncing", "camera-synced");
  previewSyncBadge.hidden = true;
}

function showPreviewSyncing() {
  if (previewSyncTimer) {
    clearTimeout(previewSyncTimer);
    previewSyncTimer = null;
  }
  previewFrame.classList.remove("camera-synced");
  previewFrame.classList.add("camera-syncing");
  previewSyncBadge.textContent = "syncing";
  previewSyncBadge.hidden = false;
}

function flashPreviewSync() {
  previewFrame.classList.remove("camera-syncing");
  previewFrame.classList.add("camera-synced");
  previewSyncBadge.textContent = "camera";
  previewSyncBadge.hidden = false;
  if (previewSyncTimer) {
    clearTimeout(previewSyncTimer);
  }
  previewSyncTimer = setTimeout(() => {
    previewFrame.classList.remove("camera-synced");
    previewSyncBadge.hidden = true;
    previewSyncTimer = null;
  }, 450);
}

function formatValueMm(valueMm) {
  if (valueMm == null) {
    return "—";
  }
  return `${valueMm.toFixed(2)} mm`;
}

function formatAngleRad(valueDeg) {
  const radians = (valueDeg * Math.PI) / 180;
  const piMultiple = radians / Math.PI;
  if (Math.abs(piMultiple - 1) < 0.01) {
    return "π rad";
  }
  if (Math.abs(piMultiple - 2) < 0.01) {
    return "2π rad";
  }
  if (Math.abs(piMultiple - 0.5) < 0.01) {
    return "π/2 rad";
  }
  return `${radians.toFixed(2)} rad`;
}

function formatParameterValue(row) {
  if (row.value_deg != null) {
    return `${row.value_deg.toFixed(1)}° (${formatAngleRad(row.value_deg)})`;
  }
  return formatValueMm(row.value_mm);
}

function updateUndoRedoButtons() {
  undoBtn.disabled = !documentHistoryState?.can_undo;
  redoBtn.disabled = !documentHistoryState?.can_redo;
}

function resetDocumentHistory() {
  documentHistoryState = null;
  updateUndoRedoButtons();
}

function acceptDocumentHistory(state) {
  documentHistoryState = state;
  updateUndoRedoButtons();
}

async function applyParameterChange(id, expr) {
  if (documentMutationBusy) throw new Error("Another document edit is in progress.");
  documentMutationBusy = true;
  cancelFeatureReview();
  const path = currentPath;
  try {
    const state = await invoke("set_document_parameter_cmd", { path, id, expr, history: documentHistoryState?.history ?? null });
    acceptDocumentHistory(state);
    await loadDocument(path, { keepParamHistory: true });
  } finally { documentMutationBusy = false; }
}

async function applyParameter(row, input) {
  const nextExpr = input.value.trim();
  if (!nextExpr || nextExpr === row.expr) {
    input.value = row.expr;
    return;
  }

  setStatus(`Updating ${row.name}…`);
  try {
    await applyParameterChange(row.id, nextExpr);
    setStatus(`Updated ${row.name}`);
  } catch (error) {
    input.value = row.expr;
    setStatus(`Error: ${error}`);
  }
}

async function undoParameterChange() {
  if (documentMutationBusy || !currentPath || !documentHistoryState?.can_undo) {
    return;
  }

  documentMutationBusy = true;
  cancelFeatureReview();
  setStatus("Undoing…");
  try {
    const state = await invoke("undo_document_cmd", {
      path: currentPath,
      history: documentHistoryState.history,
    });
    acceptDocumentHistory(state);
    await loadDocument(currentPath, { keepParamHistory: true });
    setStatus("Undid document edit");
  } catch (error) {
    setStatus(`Error: ${error}`);
  } finally { documentMutationBusy = false; }
}

async function redoParameterChange() {
  if (documentMutationBusy || !currentPath || !documentHistoryState?.can_redo) {
    return;
  }

  documentMutationBusy = true;
  cancelFeatureReview();
  setStatus("Redoing…");
  try {
    const state = await invoke("redo_document_cmd", {
      path: currentPath,
      history: documentHistoryState.history,
    });
    acceptDocumentHistory(state);
    await loadDocument(currentPath, { keepParamHistory: true });
    setStatus("Redid document edit");
  } catch (error) {
    setStatus(`Error: ${error}`);
  } finally { documentMutationBusy = false; }
}

function renderParameters(rows) {
  parameterRows = rows;
  parametersPanel.replaceChildren();

  if (!rows.length) {
    const empty = document.createElement("p");
    empty.className = "parameters-empty";
    empty.textContent = "No parameters.";
    parametersPanel.append(empty);
    return;
  }

  for (const row of rows) {
    const wrapper = document.createElement("div");
    wrapper.className = "param-row";

    const label = document.createElement("label");
    label.textContent = row.name;
    label.htmlFor = `param-${row.id}`;

    const input = document.createElement("input");
    input.id = `param-${row.id}`;
    input.type = "text";
    input.value = row.expr;
    input.placeholder = row.expr_hint ?? "";
    input.title = row.unit_hint ?? "";
    input.spellcheck = false;
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        input.blur();
      }
    });
    input.addEventListener("blur", () => {
      applyParameter(row, input).catch((error) => setStatus(`Error: ${error}`));
    });

    const value = document.createElement("span");
    value.className = "param-value";
    value.textContent = formatParameterValue(row);

    const intentBtn = document.createElement("button");
    intentBtn.type = "button";
    intentBtn.className = "param-intent";
    intentBtn.textContent = "Intent";
    intentBtn.title = `What drives ${row.name} and what an edit to it changes`;
    intentBtn.addEventListener("click", () => {
      showParameterIntent(row).catch((error) => setStatus(`Error: ${error}`));
    });

    const head = document.createElement("div");
    head.className = "param-head";
    head.append(label, intentBtn);

    wrapper.append(head, input, value);
    if (row.unit_hint) {
      const hint = document.createElement("span");
      hint.className = "param-hint";
      hint.textContent = row.unit_hint;
      wrapper.append(hint);
    }
    parametersPanel.append(wrapper);
  }
}

async function loadParameters() {
  if (!currentPath) {
    renderParameters([]);
    return;
  }
  const rows = await invoke("list_document_parameters_cmd", { path: currentPath });
  renderParameters(rows);
}

async function loadTemplates() {
  const templates = await invoke("list_templates");
  templateSelect.replaceChildren();
  for (const template of templates) {
    const option = document.createElement("option");
    option.value = template.id;
    option.textContent = template.label;
    templateSelect.append(option);
  }
}

async function loadDocument(path, options = {}) {
  if (documentMutationBusy && !options.keepParamHistory) return;
  if (!path) {
    setStatus("No document selected.");
    return;
  }

  const loadSequence = ++documentLoadSequence;
  renderSketchCreation(null);
  cancelFeatureReview();
  clearSketchDraft();
  currentPath = path;
  if (!options.keepParamHistory) {
    resetDocumentHistory();
  }
  setStatus(`Loading ${path}…`);

  // A document that no longer regenerates has no preview, but its info,
  // parameters, and regeneration inspection still load so it can be repaired.
  const [inspectResult, previewResult, regenerationResult, parametersResult, featuresResult, sketchesResult, sketchCreationResult] =
    await Promise.allSettled([
      invoke("inspect_document_cmd", { path }),
      invoke("preview_document_cmd", { path }),
      invoke("inspect_regeneration_cmd", { path }),
      options.skipParameters
        ? Promise.resolve(null)
        : invoke("list_document_parameters_cmd", { path }),
      invoke("list_document_features_cmd", { path }),
      invoke("inspect_sketches_cmd", { path }),
      invoke("inspect_sketch_creation_cmd", { path }),
    ]);
  if (loadSequence !== documentLoadSequence || currentPath !== path) return;
  renderSketchEditor(sketchesResult.status === "fulfilled" ? sketchesResult.value : null);
  renderFeatureTree(featuresResult.status === "fulfilled" ? featuresResult.value : null);
  if (inspectResult.status === "rejected") {
    throw inspectResult.reason;
  }
  const inspect = inspectResult.value;
  renderSketchCreation(sketchCreationResult.status === "fulfilled" && sketchCreationResult.value.document_id === inspect.id ? sketchCreationResult.value : null);

  renderInfo(docInfo, [
    ["Name", inspect.name],
    ["ID", inspect.id],
    ["Path", path],
    ["Sketches", inspect.sketches],
    ["Features", inspect.features],
    ["Parameters", inspect.parameters],
    ["Topo refs", inspect.semantic_refs],
  ]);

  if (regenerationResult.status === "fulfilled") {
    renderRegeneration(regenerationResult.value);
  } else {
    renderRegenerationUnavailable(regenerationResult.reason);
  }

  if (parametersResult.status === "fulfilled" && parametersResult.value) {
    renderParameters(parametersResult.value);
  } else if (parametersResult.status === "rejected") {
    renderParameters([]);
  }

  clearSelection();
  clearIntent();

  if (previewResult.status === "rejected") {
    previewSelectionFrame = null;
    preview.removeAttribute("src");
    preview.alt = "No preview";
    renderInfo(previewInfo, [["Preview", "unavailable"]]);
    setStatus(`Loaded ${inspect.name} without a preview: ${previewResult.reason}`);
    return;
  }
  const previewData = previewResult.value;
  previewSelectionFrame = previewData.selection_frame ?? null;
  preview.src = `data:image/png;base64,${previewData.png_base64}`;
  preview.alt = previewData.name;

  renderInfo(previewInfo, [
    ["Triangles", previewData.triangles],
    ["Vertices", previewData.vertices],
    [
      "Bounds min (m)",
      previewData.bounds_min_m.map((v) => v.toFixed(4)).join(", "),
    ],
    [
      "Bounds max (m)",
      previewData.bounds_max_m.map((v) => v.toFixed(4)).join(", "),
    ],
  ]);

  setStatus(`Loaded ${previewData.name}`);
}

async function openDocument() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open .ocad.d directory",
  });
  if (selected) {
    await loadDocument(selected);
  }
}

async function openViewport() {
  if (!currentPath) {
    setStatus("Open a document first.");
    return;
  }
  setStatus("Opening 3D viewport…");
  await invoke("open_viewport_cmd", { path: currentPath });
  setStatus("3D viewport open — click geometry to update Selection.");
}

async function listenViewportPicks() {
  await listen("viewport-pick", (event) => {
    handlePickSummary(event.payload, "viewport");
  });
}

async function listenPreviewSync() {
  await listen("preview-syncing", () => {
    showPreviewSyncing();
  });
  await listen("preview-synced", (event) => {
    const synced = event.payload;
    preview.src = `data:image/png;base64,${synced.png_base64}`;
    renderHighlight(synced.highlight_segments_px ?? []);
    flashPreviewSync();
  });
  await listen("preview-sync-failed", () => {
    clearPreviewSyncing();
  });
}

async function createSample() {
  const selected = await save({
    title: "Create sample document",
    defaultPath: "sample.ocad.d",
  });
  if (!selected) {
    return;
  }
  const templateId = templateSelect.value;
  await invoke("create_template_document", {
    path: selected,
    templateId,
  });
  await loadDocument(selected);
}

highlightOverlay.setAttribute("viewBox", `0 0 ${PREVIEW_WIDTH} ${PREVIEW_HEIGHT}`);

async function boot() {
  try {
    await listenViewportPicks();
    await listenPreviewSync();
    await loadTemplates();
    const defaultPath = await invoke("default_example_path");
    if (defaultPath) {
      await loadDocument(defaultPath);
    } else {
      setStatus("Open a .ocad.d directory to preview.");
      renderParameters([]);
      clearSelection();
      resetDocumentHistory();
    }
  } catch (error) {
    setStatus(`Error: ${error}`);
  }
}

openBtn.addEventListener("click", () => {
  openDocument().catch((error) => setStatus(`Error: ${error}`));
});

preview.addEventListener("click", (event) => {
  pickAtPreview(event).catch((error) => setStatus(`Error: ${error}`));
});

refreshBtn.addEventListener("click", () => {
  loadDocument(currentPath, { keepParamHistory: true }).catch((error) =>
    setStatus(`Error: ${error}`),
  );
});

undoBtn.addEventListener("click", () => {
  undoParameterChange().catch((error) => setStatus(`Error: ${error}`));
});

redoBtn.addEventListener("click", () => {
  redoParameterChange().catch((error) => setStatus(`Error: ${error}`));
});

document.addEventListener("keydown", (event) => {
  if (!(event.ctrlKey || event.metaKey)) {
    return;
  }
  const key = event.key.toLowerCase();
  if (key === "z" && !event.shiftKey) {
    event.preventDefault();
    undoParameterChange().catch((error) => setStatus(`Error: ${error}`));
    return;
  }
  if ((key === "z" && event.shiftKey) || key === "y") {
    event.preventDefault();
    redoParameterChange().catch((error) => setStatus(`Error: ${error}`));
  }
});

viewportBtn.addEventListener("click", () => {
  openViewport().catch((error) => setStatus(`Error: ${error}`));
});

createBtn.addEventListener("click", () => {
  createSample().catch((error) => setStatus(`Error: ${error}`));
});

boot();

function cancelFeatureReview() {
  cancelSketchCreationReview();
  featureReviewSequence += 1;
  reviewedFeatureEdit = null;
  document.getElementById("feature-review").hidden = true;
}

async function reviewFeatureEdit(edit, selectionEdit = null) {
  if (featureBusy || documentMutationBusy || !featureTree || !currentPath) return;
  cancelFeatureReview();
  clearSketchDraft(); drawSketch();
  featureBusy = true;
  const path = currentPath;
  const reviewSequence = featureReviewSequence;
  const revision = featureTree.revision;
  const sequence = documentLoadSequence;
  setStatus("Verifying feature edit…");
  try {
    const plan = selectionEdit
      ? await invoke("plan_selection_edit_cmd", { path, edit: selectionEdit })
      : await invoke("plan_feature_edit_cmd", { path, revision, edit });
    if (reviewSequence !== featureReviewSequence || sequence !== documentLoadSequence || path !== currentPath || revision !== featureTree?.revision) return;
    reviewedFeatureEdit = { path, revision, edit: plan.edit, selectionEdit };
    const volume = plan.evidence.volume_m3;
    const admitted = plan.edit;
    const detail = admitted.operation === "set_expression" ? `${admitted.field} = ${admitted.expr}`
      : admitted.operation === "create_extrude" ? `${admitted.sketch_feature} / ${admitted.profile_ref}, length ${admitted.length_expr}, new body`
      : admitted.operation === "move" ? `position ${Object.entries(admitted.position).map(([key, value]) => `${key} ${value}`).join(" ")}`
      : admitted.suppressed ? "suppress" : "enable";

    document.getElementById("feature-effect").textContent = `${admitted.id}: ${detail}. Result: ${plan.evidence.no_op ? "no source change" : volume == null ? "no solid body" : `${(volume * 1e9).toFixed(3)} mm³`}. ${plan.evidence.no_op ? "No regeneration needed." : "Geometry, display mesh and required assertions verified."} Source changes only when Apply is pressed.`;
    document.getElementById("feature-review").hidden = false;
    setStatus("Edit verified. Review the result and apply.");
  } catch (error) {
    if (reviewSequence === featureReviewSequence && sequence === documentLoadSequence && path === currentPath) setStatus(`Edit refused: ${error}`);
  }
  finally { featureBusy = false; }
}

function renderFeatureTree(tree) {
  featureTree = tree;
  cancelFeatureReview();
  const panel = document.getElementById("feature-tree");
  const profiles = document.getElementById("feature-profile");
  panel.replaceChildren(); profiles.replaceChildren();
  document.getElementById("feature-create-review").disabled = !tree;
  if (!tree) { panel.textContent = "Feature editing requires a part document."; return; }
  tree.rows.forEach((row, index) => {
    const wrapper = document.createElement("div"); wrapper.className = "feature-row"; wrapper.dataset.featureId = row.id;
    const title = document.createElement("strong");
    title.textContent = `${row.name} (${row.feature_type})${row.suppressed ? " — suppressed" : ""}`;
    const identity = document.createElement("small"); identity.textContent = row.id;
    wrapper.append(title, identity);
    for (const field of row.fields) {
      const label = document.createElement("label"); label.textContent = `${field.field} — ${field.unit_hint}`;
      const input = document.createElement("input"); input.value = field.expr; input.dataset.field = field.field;
      input.addEventListener("input", cancelFeatureReview);
      const button = document.createElement("button"); button.type = "button"; button.textContent = "Review dimension";
      button.addEventListener("click", () => reviewFeatureEdit({ operation: "set_expression", id: row.id, field: field.field, expr: input.value.trim() }));
      label.append(input); wrapper.append(label, button);
    }
    const action = (text, edit, disabled = false) => {
      const button = document.createElement("button"); button.type = "button"; button.textContent = text; button.disabled = disabled;
      button.addEventListener("click", () => reviewFeatureEdit(edit)); wrapper.append(button);
    };
    action(row.suppressed ? "Review enable" : "Review suppress", { operation: "suppress", id: row.id, suppressed: !row.suppressed });
    action("Review move up", { operation: "move", id: row.id, position: { before: tree.rows[index - 1]?.id } }, index === 0);
    action("Review move down", { operation: "move", id: row.id, position: { after: tree.rows[index + 1]?.id } }, index === tree.rows.length - 1);
    panel.append(wrapper);
    row.closed_profiles.forEach(profile => {
      const option = document.createElement("option"); option.value = JSON.stringify({ sketch_feature: row.id, profile_ref: profile });
      option.textContent = `${row.name}: ${profile}`; profiles.append(option);
    });
  });
  document.getElementById("feature-create-review").disabled = profiles.options.length === 0;
}

document.getElementById("feature-create").addEventListener("input", cancelFeatureReview);
document.getElementById("feature-create").addEventListener("submit", event => {
  event.preventDefault();
  const profile = document.getElementById("feature-profile").value;
  if (!profile) return;
  reviewFeatureEdit({ operation: "create_extrude", ...JSON.parse(profile), id: document.getElementById("feature-id").value.trim(), name: document.getElementById("feature-name").value.trim(), length_expr: document.getElementById("feature-length").value.trim() });
});
document.getElementById("feature-cancel").addEventListener("click", cancelFeatureReview);
document.getElementById("feature-apply").addEventListener("click", async () => {
  if (featureBusy || documentMutationBusy || !reviewedFeatureEdit) return;
  const reviewed = reviewedFeatureEdit;
  if (reviewed.path !== currentPath || reviewed.revision !== featureTree?.revision) { cancelFeatureReview(); return; }
  featureBusy = true;
  documentMutationBusy = true;
  try {
    const result = reviewed.selectionEdit
      ? await invoke("apply_selection_edit_cmd", { path: reviewed.path, edit: reviewed.selectionEdit, history: documentHistoryState?.history ?? null })
      : await invoke("apply_feature_edit_cmd", { path: reviewed.path, revision: reviewed.revision, edit: reviewed.edit, history: documentHistoryState?.history ?? null });
    if (currentPath === reviewed.path) {
      acceptDocumentHistory(result.history);
      await loadDocument(currentPath, { keepParamHistory: true });
      setStatus("Applied verified feature edit.");
    }
  } catch (error) { setStatus(`Edit refused: ${error}`); }
  finally { featureBusy = false; documentMutationBusy = false; cancelFeatureReview(); }
});

function renderSelectedDimensionForm() {
  const form = document.getElementById("selection-edit");
  form.hidden = !selectedBinding;
  if (!selectedBinding) return;
  document.getElementById("selection-source").textContent = `Source: ${selectedBinding.target.kind} → ${selectedBinding.feature_id}`;
  const fields = document.getElementById("selection-field"); fields.replaceChildren();
  selectedBinding.fields.forEach(field => {
    const option = document.createElement("option"); option.value = field.field; option.textContent = `${field.field} (${field.unit_hint})`; fields.append(option);
  });
  updateSelectedDimensionExpression();
}
function updateSelectedDimensionExpression() {
  cancelFeatureReview();
  const field = selectedBinding?.fields.find(f => f.field === document.getElementById("selection-field").value);
  document.getElementById("selection-expr").value = field?.expr ?? "";
}
document.getElementById("selection-field").addEventListener("change", updateSelectedDimensionExpression);
document.getElementById("selection-expr").addEventListener("input", cancelFeatureReview);
document.getElementById("selection-edit").addEventListener("submit", event => {
  event.preventDefault();
  if (!selectedBinding || !selectedFrame) return;
  const edit = { source_revision: selectedFrame.source_revision, scene_revision: selectedFrame.scene_revision, target: selectedBinding.target, field: document.getElementById("selection-field").value, expr: document.getElementById("selection-expr").value.trim() };
  reviewFeatureEdit(null, edit);
});

// Disposable sketch presentation/drafts. All source changes go to typed backend forms.
let sketchEditorState = null;
let sketchDraft = null;
let reviewedSketch = null;
let sketchReviewSequence = 0;
let sketchBusy = false;
function drawLineDraft(start, end) {
  const e=document.createElementNS("http://www.w3.org/2000/svg","line"); e.id="sketch-draft-line"; e.setAttribute("class","sketch-geometry construction");
  const number=s=>Number(s.split(" ")[0]); for(const [k,v] of Object.entries({x1:number(start.x),y1:-number(start.y),x2:number(end.x),y2:-number(end.y)}))e.setAttribute(k,String(v));
  document.getElementById("sketch-draft-line")?.remove(); sketchCanvas.append(e);
}
let lineStart = null;
let circleStart = null;
let rectangleStart = null;
let arcCenter = null;
let arcStart = null;
let ellipseCenter = null;
let ellipseMajor = null;
let sketchConstraintSelection = [];
const sketchCanvas = document.getElementById("sketch-canvas");
function ellipsePreview(center,major,through) {
  const mm=value=>Number(value.split(" ")[0]);
  const c={x:mm(center.x),y:mm(center.y)},m={x:mm(major.x),y:mm(major.y)},q={x:mm(through.x),y:mm(through.y)};
  const a=Math.hypot(m.x-c.x,m.y-c.y);
  if(!Number.isFinite(a)||a<=0.000001||a>1e9)return null;
  const ux=(m.x-c.x)/a,uy=(m.y-c.y)/a,x=(q.x-c.x)*ux+(q.y-c.y)*uy,y=(q.y-c.y)*ux-(q.x-c.x)*uy,k=1-(x/a)**2;
  if(!Number.isFinite(k)||k<=1e-9)return null;
  const b=Math.abs(y)/Math.sqrt(k);
  if(!Number.isFinite(b)||b<=0.000001||b>a)return null;
  return {c,a,b,angle:Math.atan2(m.y-c.y,m.x-c.x)};
}
function drawEllipseDraft(center,major,through) {
  document.getElementById("sketch-draft-ellipse")?.remove();
  const ns="http://www.w3.org/2000/svg",group=document.createElementNS(ns,"g");group.id="sketch-draft-ellipse";
  const add=(tag,attrs)=>{const e=document.createElementNS(ns,tag);for(const [key,value] of Object.entries({...attrs,class:"sketch-geometry construction"}))e.setAttribute(key,String(value));group.append(e);};
  const mm=value=>Number(value.split(" ")[0]);
  add("circle",{cx:mm(center.x),cy:-mm(center.y),r:Number(document.getElementById("sketch-span").value)/150});
  if(major) {
    add("line",{x1:mm(center.x),y1:-mm(center.y),x2:mm(major.x),y2:-mm(major.y)});
    const preview=ellipsePreview(center,major,through);
    if(preview)add("ellipse",{cx:preview.c.x,cy:-preview.c.y,rx:preview.a,ry:preview.b,transform:`rotate(${-preview.angle*180/Math.PI} ${preview.c.x} ${-preview.c.y})`});
  }
  sketchCanvas.append(group);
}
function arcPreview(center,start,direction) {
  const mm=value=>Number(value.split(" ")[0]);
  const c={x:mm(center.x),y:mm(center.y)},s={x:mm(start.x),y:mm(start.y)},d={x:mm(direction.x),y:mm(direction.y)};
  const r=Math.hypot(s.x-c.x,s.y-c.y),a=Math.atan2(s.y-c.y,s.x-c.x),b=Math.atan2(d.y-c.y,d.x-c.x);
  const clockwise=document.getElementById("sketch-arc-direction").value==="cw",tau=2*Math.PI;
  const sweep=((clockwise?a-b:b-a)%tau+tau)%tau;
  if(!Number.isFinite(r)||r<=0.000001||r>1000000||Math.hypot(d.x-c.x,d.y-c.y)<=0.000001||sweep<=1e-9||sweep>=tau-1e-9)return null;
  return {c,s,end:{x:c.x+r*Math.cos(b),y:c.y+r*Math.sin(b)},r,sweep,clockwise};
}
function arcPath(start,end,r,sweep,clockwise=false) {
  return `M ${start.x} ${-start.y} A ${r} ${r} 0 ${sweep>Math.PI?1:0} ${clockwise?1:0} ${end.x} ${-end.y}`;
}
function drawArcDraft(center,start,direction) {
  document.getElementById("sketch-draft-arc")?.remove();
  const ns="http://www.w3.org/2000/svg",group=document.createElementNS(ns,"g");group.id="sketch-draft-arc";
  const add=(tag,attributes)=>{const e=document.createElementNS(ns,tag);for(const [key,value] of Object.entries({...attributes,class:"sketch-geometry construction"}))e.setAttribute(key,String(value));group.append(e);};
  const mm=value=>Number(value.split(" ")[0]);
  add("circle",{cx:mm(center.x),cy:-mm(center.y),r:Number(document.getElementById("sketch-span").value)/150});
  if(start) {
    add("line",{x1:mm(center.x),y1:-mm(center.y),x2:mm(start.x),y2:-mm(start.y)});
    const preview=arcPreview(center,start,direction);
    if(preview) {
      add("path",{d:arcPath(preview.s,preview.end,preview.r,preview.sweep,preview.clockwise)});
      if(document.getElementById("sketch-arc-sector").checked)add("line",{x1:preview.c.x,y1:-preview.c.y,x2:preview.end.x,y2:-preview.end.y});
    }
  }
  sketchCanvas.append(group);
}
function rectangleSize(start,end) {
  const mm=value=>Number(value.split(" ")[0]);
  return {x:Math.min(mm(start.x),mm(end.x)),y:-Math.max(mm(start.y),mm(end.y)),width:Math.abs(mm(end.x)-mm(start.x)),height:Math.abs(mm(end.y)-mm(start.y))};
}
function drawRectangleDraft(start,end) {
  document.getElementById("sketch-draft-rectangle")?.remove();
  const e=document.createElementNS("http://www.w3.org/2000/svg","rect");e.id="sketch-draft-rectangle";
  for(const [key,value] of Object.entries({...rectangleSize(start,end),class:"sketch-geometry construction"}))e.setAttribute(key,String(value));
  sketchCanvas.append(e);
}
function circleRadius(start, end) {
  const mm=value=>Number(value.split(" ")[0]);
  return Math.hypot(mm(end.x)-mm(start.x),mm(end.y)-mm(start.y));
}
function drawCircleDraft(start, end) {
  document.getElementById("sketch-draft-circle")?.remove();
  const mm=value=>Number(value.split(" ")[0]);
  const group=document.createElementNS("http://www.w3.org/2000/svg","g");group.id="sketch-draft-circle";
  for(const radius of [circleRadius(start,end),Number(document.getElementById("sketch-span").value)/150]) {
    const circle=document.createElementNS("http://www.w3.org/2000/svg","circle");
    for(const [key,value] of Object.entries({cx:mm(start.x),cy:-mm(start.y),r:radius,class:"sketch-geometry construction"}))circle.setAttribute(key,String(value));
    group.append(circle);
  }
  sketchCanvas.append(group);
}
function sketchRow() { return sketchEditorState?.sketches.find(row => row.source.id === document.getElementById("sketch-select").value); }
function clearSketchDraft(keepConstraintSelection = false) {
  cancelSketchCreationReview();
  sketchReviewSequence++; reviewedSketch = null; sketchDraft = null; lineStart = null; circleStart = null; rectangleStart = null; arcCenter = null; arcStart = null; ellipseCenter = null; ellipseMajor = null;
  document.getElementById("sketch-draft-circle")?.remove();
  document.getElementById("sketch-draft-rectangle")?.remove();
  document.getElementById("sketch-draft-arc")?.remove();
  document.getElementById("sketch-draft-ellipse")?.remove();
  document.getElementById("sketch-review").hidden = true;
  if (!keepConstraintSelection) {
    sketchConstraintSelection=[];
    for(const id of ["constraint-target-a","constraint-target-b","constraint-remove"])document.getElementById(id).value="";
  }
}
function sketchSolveLabel(state) {
  if(state.status==="under_constrained")return `Underconstrained · ${state.dof} DOF`;
  if(state.status==="fully_constrained")return "Fully constrained · 0 DOF";
  if(state.status==="over_constrained")return `Overconstrained · ${state.redundant} redundant constraints`;
  return state.status;
}
function renderSketchConstraintControls(row) {
  const kind=document.getElementById("constraint-kind").value,pair=kind==="distance"||kind==="coincident",remove=kind==="remove_constraint";
  const targets=row?.constraint_targets;
  const ids=remove?[]:(pair?targets?.points:kind==="radius"?targets?.circles:targets?.lines)??[];
  const label=id=>{
    const entity=row.solved.entities.find(e=>e.id===id);
    return entity?.type==="point"?`${id} · (${(entity.x*1000).toFixed(3)}, ${(entity.y*1000).toFixed(3)}) mm`:id;
  };
  const fill=(id,options,describe)=>{
    const select=document.getElementById(id),old=select.value;select.replaceChildren();
    const empty=document.createElement("option");empty.value="";empty.textContent="Choose a target";select.append(empty);
    for(const value of options){const option=document.createElement("option");option.value=value;option.textContent=describe(value);select.append(option);}
    select.value=options.includes(old)?old:"";
  };
  fill("constraint-target-a",ids,label);fill("constraint-target-b",pair?ids:[],label);
  fill("constraint-remove",row?.source.constraints.map(c=>c.id)??[],id=>{const c=row.source.constraints.find(c=>c.id===id);return `${c.type} · ${id}${c.expr?` · ${c.expr}`:""}`;});
  document.getElementById("constraint-a-label").hidden=remove;
  document.getElementById("constraint-b-label").hidden=!pair;
  document.getElementById("constraint-value-label").hidden=kind!=="distance"&&kind!=="radius";
  document.getElementById("constraint-remove-label").hidden=!remove;
  document.getElementById("constraint-review").disabled=!row;
  sketchConstraintSelection=[document.getElementById("constraint-target-a").value,document.getElementById("constraint-target-b").value].filter(Boolean);
}
function selectSketchConstraintTarget(id,extend) {
  const row=sketchRow(),targets=row?.constraint_targets;if(!targets)return;
  const old=[...sketchConstraintSelection];clearSketchDraft(true);
  const kind=document.getElementById("constraint-kind");
  if(targets.circles.includes(id))kind.value="radius";
  else if(targets.lines.includes(id)){if(!["horizontal","vertical"].includes(kind.value))kind.value="horizontal";}
  else if(targets.points.includes(id)){if(!["distance","coincident"].includes(kind.value))kind.value="distance";}
  else {clearSketchDraft();drawSketch();return;}
  renderSketchConstraintControls(row);
  document.getElementById("constraint-target-a").value=extend&&targets.points.includes(id)&&old.length&&targets.points.includes(old[0])?old[0]:id;
  document.getElementById("constraint-target-b").value=extend&&targets.points.includes(id)&&old.length&&id!==old[0]?id:"";
  drawSketch();
}
function renderSketchEditor(state) {
  sketchEditorState = state;
  const select = document.getElementById("sketch-select"); const old = select.value; select.replaceChildren();
  state?.sketches.forEach(row => { const o=document.createElement("option"); o.value=row.source.id; o.textContent=`${row.source.name} · ${row.workplane_label}`; select.append(o); });
  if ([...select.options].some(o=>o.value===old)) select.value=old;
  drawSketch();
}
function drawSketch(row = sketchRow()) {
  sketchCanvas.replaceChildren(); if (!row) { document.getElementById("sketch-diagnostics").textContent="No admitted source sketch."; renderSketchConstraintControls(null); return; }
  const span=Number(document.getElementById("sketch-span").value); if (!Number.isFinite(span)||span<=0) return;
  sketchCanvas.setAttribute("viewBox", `${-span/2} ${-span/3} ${span} ${span*2/3}`);
  const entities=row.solved.entities;
  const points=new Map(entities.filter(e=>e.type==="point").map(e=>[e.id,e]));
  const element=(tag,attrs,id=null)=> { const e=document.createElementNS("http://www.w3.org/2000/svg",tag); Object.entries(attrs).forEach(([k,v])=>e.setAttribute(k,String(v))); if(id)e.dataset.sourceId=id; sketchCanvas.append(e); return e; };
  element("line",{x1:-span/2,y1:0,x2:span/2,y2:0,class:"sketch-axis"}); element("line",{x1:0,y1:-span/3,x2:0,y2:span/3,class:"sketch-axis"});
  for(const e of entities) {
    const style={class:e.construction?"sketch-geometry construction":"sketch-geometry"};
    if(e.type==="line") { const a=points.get(e.start),b=points.get(e.end); if(a&&b) element("line",{...style,x1:a.x*1000,y1:-a.y*1000,x2:b.x*1000,y2:-b.y*1000},e.id); }
    if(e.type==="arc") {
      const a=points.get(e.start_point),b=points.get(e.end_point),sweep=((e.end_angle-e.start_angle)%(2*Math.PI)+2*Math.PI)%(2*Math.PI);
      if(a&&b&&Number.isFinite(e.radius)&&sweep>0)element("path",{...style,d:arcPath({x:a.x*1000,y:a.y*1000},{x:b.x*1000,y:b.y*1000},e.radius*1000,sweep)},e.id);
    }
    if(e.type==="circle"||e.type==="ellipse") { const p=points.get(e.center); if(!p)continue;
      if(e.type==="circle") element("circle",{...style,cx:p.x*1000,cy:-p.y*1000,r:e.radius*1000},e.id);
      else element("ellipse",{...style,cx:p.x*1000,cy:-p.y*1000,rx:e.major_radius*1000,ry:e.minor_radius*1000,transform:`rotate(${-e.axis_angle*180/Math.PI} ${p.x*1000} ${-p.y*1000})`},e.id);
    }
  }
  const authored=new Set(row.source.entities.filter(e=>e.type==="point").map(e=>e.id));
  for(const p of points.values()) if(authored.has(p.id)) element("circle",{cx:p.x*1000,cy:-p.y*1000,r:span/150,class:"sketch-point"},p.id);
  renderSketchConstraintControls(sketchRow());
  for(const element of sketchCanvas.querySelectorAll("[data-source-id]"))if(sketchConstraintSelection.includes(element.dataset.sourceId))element.classList.add("sketch-selected");
  document.getElementById("sketch-diagnostics").textContent=`${row.workplane_label} · origin ${row.resolved_frame.origin_m.map(v=>(v*1000).toFixed(3)).join(", ")} mm · ${sketchSolveLabel(row.solved.solve_state)} · ${(row.solved.profiles??[]).length} profiles · ${row.source.constraints.length} constraints. Arcs support pointer creation and construction selection; radius/angle field editing, cubic and stored rectangle-helper editing remain outside this form scope.`;
}
function draftPoint(event) {
  const p=sketchCanvas.createSVGPoint(); p.x=event.clientX;p.y=event.clientY;const local=p.matrixTransform(sketchCanvas.getScreenCTM().inverse());
  // ViewBox coordinates explicitly labelled mm; the backend evaluates these unit-bearing form strings.
  return {x:`${local.x.toFixed(3)} mm`,y:`${(-local.y).toFixed(3)} mm`};
}
sketchCanvas.addEventListener("click",event=> {
  if(documentMutationBusy||sketchBusy||!sketchRow())return;
  reviewedSketch=null; sketchReviewSequence++; document.getElementById("sketch-review").hidden=true;
  const p=draftPoint(event),tool=document.getElementById("sketch-tool").value;
  if(tool==="ellipse") {
    if(!ellipseCenter) {
      sketchDraft=null;lineStart=null;circleStart=null;rectangleStart=null;arcCenter=null;arcStart=null;drawSketch();ellipseCenter=p;drawEllipseDraft(p,null,p);
      setStatus("Choose ellipse major-axis endpoint (local mm).");return;
    }
    if(!ellipseMajor) {
      const a=circleRadius(ellipseCenter,p);
      if(!Number.isFinite(a)||a<=0.000001||a>1e9) {clearSketchDraft();drawSketch();setStatus("Ellipse refused: choose a finite major radius above 1 nm and at most 1e6 m.");return;}
      ellipseMajor=p;drawEllipseDraft(ellipseCenter,p,p);setStatus("Choose a point on the ellipse inside the major-axis span.");return;
    }
    if(!ellipsePreview(ellipseCenter,ellipseMajor,p)) {clearSketchDraft();drawSketch();setStatus("Ellipse refused: choose an unambiguous point on the ellipse with 0 < minor radius <= major radius.");return;}
    const used=new Set(sketchRow().source.entities.map(e=>e.id));let n=1;
    while(["","_center"].some(suffix=>used.has(`ent:ellipse_${n}${suffix}`)))n++;
    const id=`ent:ellipse_${n}`;
    sketchDraft=[{operation:"ellipse_axes",id,center_x:ellipseCenter.x,center_y:ellipseCenter.y,major_x:ellipseMajor.x,major_y:ellipseMajor.y,through_x:p.x,through_y:p.y,construction:document.getElementById("sketch-construction").checked}];
    drawEllipseDraft(ellipseCenter,ellipseMajor,p);ellipseCenter=null;ellipseMajor=null;
    document.getElementById("sketch-id").value=id;setStatus("Ellipse drafted. Review before committing.");return;
  }
  if(tool==="arc") {
    if(!arcCenter) {
      sketchDraft=null;lineStart=null;circleStart=null;rectangleStart=null;drawSketch();arcCenter=p;drawArcDraft(p,null,p);
      setStatus("Choose arc start point (local mm).");return;
    }
    if(!arcStart) {
      const radius=circleRadius(arcCenter,p);
      if(!Number.isFinite(radius)||radius<=0.000001||radius>1000000) {clearSketchDraft();drawSketch();setStatus("Arc refused: choose a finite radius above 1 nm and at most 1000 m.");return;}
      arcStart=p;drawArcDraft(arcCenter,p,p);setStatus("Choose arc end direction; the radius stays fixed.");return;
    }
    if(!arcPreview(arcCenter,arcStart,p)) {clearSketchDraft();drawSketch();setStatus("Arc refused: choose distinct centre/start/end directions and a positive sub-full turn.");return;}
    const used=new Set(sketchRow().source.entities.map(e=>e.id));let n=1;
    while(["","_center","_start","_end","_start_ray","_end_ray"].some(suffix=>used.has(`ent:arc_${n}${suffix}`)))n++;
    const id=`ent:arc_${n}`;
    sketchDraft=[{operation:"arc",id,center_x:arcCenter.x,center_y:arcCenter.y,start_x:arcStart.x,start_y:arcStart.y,end_direction_x:p.x,end_direction_y:p.y,clockwise:document.getElementById("sketch-arc-direction").value==="cw",close_sector:document.getElementById("sketch-arc-sector").checked,construction:document.getElementById("sketch-construction").checked}];
    drawArcDraft(arcCenter,arcStart,p);arcCenter=null;arcStart=null;
    document.getElementById("sketch-id").value=id;setStatus("Arc drafted. Review before committing.");return;
  }
  if(tool==="rectangle") {
    if(!rectangleStart) {
      sketchDraft=null;lineStart=null;circleStart=null;arcCenter=null;arcStart=null;drawSketch();rectangleStart=p;
      setStatus("Choose opposite rectangle corner (local mm).");return;
    }
    const size=rectangleSize(rectangleStart,p);
    if(!Number.isFinite(size.width)||!Number.isFinite(size.height)||size.width<=0||size.height<=0) {
      clearSketchDraft();drawSketch();setStatus("Rectangle refused: choose finite positive width and height (mm).");return;
    }
    const used=new Set(sketchRow().source.entities.map(e=>e.id));let n=1;
    while(used.has(`ent:rectangle_${n}`)||[0,1,2,3].some(i=>used.has(`ent:rectangle_${n}_p${i}`)||used.has(`ent:rectangle_${n}_e${i}`)))n++;
    const id=`ent:rectangle_${n}`;
    sketchDraft=[{operation:"rectangle",id,x1:rectangleStart.x,y1:rectangleStart.y,x2:p.x,y2:p.y,construction:document.getElementById("sketch-construction").checked}];
    drawRectangleDraft(rectangleStart,p);rectangleStart=null;
    document.getElementById("sketch-id").value=id;setStatus("Rectangle drafted. Review before committing.");return;
  }
  if(tool==="circle") {
    if(!circleStart) {
      sketchDraft=null;lineStart=null;rectangleStart=null;arcCenter=null;arcStart=null;drawSketch();circleStart=p;drawCircleDraft(p,p);
      setStatus("Choose circle radius point (local mm).");return;
    }
    const radius=circleRadius(circleStart,p);
    if(!Number.isFinite(radius)||radius<=0) {clearSketchDraft();drawSketch();setStatus("Circle refused: choose a finite positive radius (mm).");return;}
    const used=new Set(sketchRow().source.entities.map(e=>e.id));let n=1;
    while([`ent:circle_${n}`,`ent:circle_${n}_center`].some(id=>used.has(id)))n++;
    const id=`ent:circle_${n}`,center=id+"_center",radiusExpr=`${radius.toFixed(6)} mm`;
    sketchDraft=[{operation:"point",id:center,...circleStart,create:true},{operation:"circle",id,center,radius:radiusExpr,construction:document.getElementById("sketch-construction").checked,create:true}];
    drawCircleDraft(circleStart,p);circleStart=null;
    document.getElementById("sketch-action").value="circle";document.getElementById("sketch-id").value=id;
    document.getElementById("sketch-a").value=center;document.getElementById("sketch-x").value=radiusExpr;document.getElementById("sketch-create").checked=true;
    setStatus("Circle drafted. Review before committing.");return;
  }
  if(tool==="line") {
    if(!lineStart){lineStart=p;setStatus("Choose line endpoint (local mm).");return;}
    const source=sketchRow().source, used=new Set(source.entities.map(e=>e.id)); let n=1;while([`ent:line_${n}`,`ent:line_${n}_a`,`ent:line_${n}_b`].some(id=>used.has(id)))n++;
    const id=`ent:line_${n}`; sketchDraft=[{operation:"point",id:id+"_a",...lineStart,create:true},{operation:"point",id:id+"_b",...p,create:true},{operation:"line",id,start:id+"_a",end:id+"_b",construction:document.getElementById("sketch-construction").checked}];lineStart=null;
    drawLineDraft(sketchDraft[0], sketchDraft[1]);
    document.getElementById("sketch-id").value=id;setStatus("Line drafted. Review before committing."); return;
  }
  const id=event.target.dataset.sourceId;
  const source=sketchRow().source.entities.find(e=>e.id===id);
  if(source)selectSketchConstraintTarget(id,event.shiftKey);
  if(source?.type==="point") { document.getElementById("sketch-action").value="point";document.getElementById("sketch-id").value=id;document.getElementById("sketch-create").checked=false; }
  else if(source?.type==="line"||source?.type==="arc") { document.getElementById("sketch-action").value="construction";document.getElementById("sketch-id").value=id;document.getElementById("sketch-a").value=id;document.getElementById("sketch-construction").checked=source.construction;sketchDraft=null;return; }
  else if(source?.type==="circle"||source?.type==="ellipse") {
    document.getElementById("sketch-action").value=source.type;document.getElementById("sketch-id").value=id;document.getElementById("sketch-a").value=source.center;document.getElementById("sketch-create").checked=false;
    const expr=v=>typeof v==="number"?`${v*1000} mm`:v;
    document.getElementById("sketch-x").value=expr(source.radius??source.major_radius);document.getElementById("sketch-y").value=expr(source.minor_radius??0);document.getElementById("sketch-angle").value=typeof source.axis_angle==="number"?`${source.axis_angle} rad`:source.axis_angle??"0 deg";document.getElementById("sketch-construction").checked=source.construction;sketchDraft=null;return;
  }
  if(document.getElementById("sketch-action").value==="point") {document.getElementById("sketch-x").value=p.x;document.getElementById("sketch-y").value=p.y;sketchDraft=null;}
});
sketchCanvas.addEventListener("pointermove", event=>{
  if(sketchBusy||documentMutationBusy)return;
  if(lineStart)drawLineDraft(lineStart,draftPoint(event));
  if(circleStart)drawCircleDraft(circleStart,draftPoint(event));
  if(rectangleStart)drawRectangleDraft(rectangleStart,draftPoint(event));
  if(arcCenter)drawArcDraft(arcCenter,arcStart,draftPoint(event));
  if(ellipseCenter)drawEllipseDraft(ellipseCenter,ellipseMajor,draftPoint(event));
});
function sketchForm() {
  const value=id=>document.getElementById("sketch-"+id).value.trim(),operation=value("action"),id=value("id"),create=document.getElementById("sketch-create").checked,construction=document.getElementById("sketch-construction").checked;
  switch(operation){
    case "point":return {operation,id,x:value("x"),y:value("y"),create};
    case "circle":return {operation,id,center:value("a"),radius:value("x"),construction,create};
    case "ellipse":return {operation,id,center:value("a"),major_radius:value("x"),minor_radius:value("y"),axis_angle:value("angle"),construction,create};
    case "construction":return {operation,id,construction};
    case "horizontal":case "vertical":return {operation,id,line:value("a")};
    case "coincident":return {operation,id,a:value("a"),b:value("b")};
    case "distance":return {operation,id,a:value("a"),b:value("b"),expr:value("x")};
    case "radius":return {operation,id,target:value("a"),expr:value("x")};
    case "remove_constraint":return {operation,id};
  }
}
document.getElementById("sketch-form").addEventListener("submit",async event=> {
  event.preventDefault();if(sketchBusy||documentMutationBusy||!currentPath||!sketchRow())return;
  if(circleStart) {setStatus("Choose circle radius point or discard the draft before review.");return;}
  if(rectangleStart) {setStatus("Choose opposite rectangle corner or discard the draft before review.");return;}
  if(arcCenter||(document.getElementById("sketch-tool").value==="arc"&&!sketchDraft)) {setStatus("Finish arc centre/start/end direction, or choose Select to use the numeric fields.");return;}
  if(ellipseCenter||(document.getElementById("sketch-tool").value==="ellipse"&&!sketchDraft)) {setStatus("Finish ellipse centre/major endpoint/point on ellipse, or choose Select to use the numeric fields.");return;}
  await reviewSketchOperations(sketchDraft??[sketchForm()]);
});
async function reviewSketchOperations(operations) {
  cancelFeatureReview();const seq=++sketchReviewSequence,path=currentPath;
  const edit={document_id:sketchEditorState.document_id,revision:sketchEditorState.revision,sketch_id:sketchRow().source.id,operations};sketchBusy=true;
  try {const plan=await invoke("plan_sketch_edit_cmd",{path,edit});if(seq!==sketchReviewSequence||path!==currentPath||documentMutationBusy)return;
    reviewedSketch={path,edit}; const row=plan.state.sketches.find(r=>r.source.id===edit.sketch_id);drawSketch(row);
    document.getElementById("sketch-effect").textContent=`${sketchSolveLabel(row.solved.solve_state)}. Verified volume: ${plan.evidence.volume_m3==null?"—":(plan.evidence.volume_m3*1e9).toFixed(3)+" mm³"}. ${edit.operations.length} operations; source unchanged until Apply.`;document.getElementById("sketch-review").hidden=false;
  }catch(error){if(seq===sketchReviewSequence){clearSketchDraft();drawSketch();setStatus(`Sketch refused: ${error}`);}}finally{sketchBusy=false;}
}
document.getElementById("sketch-constraint-form").addEventListener("submit",async event=>{
  event.preventDefault();if(sketchBusy||documentMutationBusy||!currentPath||!sketchRow())return;
  const row=sketchRow(),operation=document.getElementById("constraint-kind").value,a=document.getElementById("constraint-target-a").value,b=document.getElementById("constraint-target-b").value,expr=document.getElementById("constraint-value").value.trim();
  let form={operation,id:row.suggested_constraint_id};
  if(operation==="remove_constraint") {
    form.id=document.getElementById("constraint-remove").value;
    if(!row.source.constraints.some(c=>c.id===form.id)){setStatus("Choose an existing constraint.");return;}
  }else {
    if(!a||(["distance","coincident"].includes(operation)&&(!b||a===b))){setStatus("Choose the required distinct source targets.");return;}
    if(operation==="horizontal"||operation==="vertical")form.line=a;
    else if(operation==="radius"){form.target=a;form.expr=expr;}
    else {form.a=a;form.b=b;if(operation==="distance")form.expr=expr;}
  }
  clearSketchDraft(true);document.getElementById("sketch-tool").value="select";drawSketch();
  await reviewSketchOperations([form]);
});
for(const id of ["constraint-kind","constraint-target-a","constraint-target-b","constraint-remove"])document.getElementById(id).addEventListener("change",()=>{clearSketchDraft(true);drawSketch();});
document.getElementById("constraint-value").addEventListener("input",()=>{clearSketchDraft(true);drawSketch();});
document.getElementById("sketch-apply").addEventListener("click",async()=> {
  if(sketchBusy||documentMutationBusy||!reviewedSketch)return;const reviewed=reviewedSketch;
  if(reviewed.path!==currentPath||reviewed.edit.revision!==featureTree?.revision){clearSketchDraft();return;}
  sketchBusy=true;documentMutationBusy=true;
  try {const result=await invoke("apply_sketch_edit_cmd",{path:reviewed.path,edit:reviewed.edit,history:documentHistoryState?.history??null});acceptDocumentHistory(result.history);await loadDocument(reviewed.path,{keepParamHistory:true});setStatus("Applied verified sketch edit.");}
  catch(error){setStatus(`Sketch refused: ${error}`);}finally{sketchBusy=false;documentMutationBusy=false;clearSketchDraft();drawSketch();}
});
["sketch-select","sketch-tool","sketch-span","sketch-arc-direction","sketch-arc-sector"].forEach(id=>document.getElementById(id).addEventListener("change",()=>{clearSketchDraft();drawSketch();}));
document.getElementById("sketch-form").addEventListener("input",()=>{clearSketchDraft();drawSketch();});
document.getElementById("sketch-clear").addEventListener("click",()=>{clearSketchDraft();drawSketch();});
document.addEventListener("keydown",event=>{
  if(event.key==="Escape"&&(circleStart||rectangleStart||arcCenter||ellipseCenter||sketchDraft||reviewedSketch||sketchConstraintSelection.length)) {clearSketchDraft();drawSketch();setStatus("Discarded sketch draft.");}
});

// Creation drafts contain only typed intent; the backend owns IDs and source.
let sketchCreationState = null;
let reviewedSketchCreation = null;
let sketchCreationSequence = 0;
let sketchCreateBusy = false;
function cancelSketchCreationReview() {
  sketchCreationSequence++;
  reviewedSketchCreation = null;
  document.getElementById("sketch-new-preview").hidden = true;
}
function renderSketchCreation(state) {
  cancelSketchCreationReview();
  sketchCreationState = state;
  const select = document.getElementById("sketch-new-plane"), previous = select.value;
  select.replaceChildren();
  state?.planes.forEach(plane => {
    const option = document.createElement("option");
    option.value = plane; option.textContent = plane; select.append(option);
  });
  state?.datum_planes.forEach(plane => {
    const option = document.createElement("option");
    option.value = `datum:${plane.feature_id}`; option.textContent = `Datum · ${plane.name}`; select.append(option);
  });
  if ([...select.options].some(option => option.value === previous)) select.value = previous;
  document.getElementById("sketch-new-review").disabled = !state || state.revision !== featureTree?.revision;
}
document.getElementById("sketch-new-form").addEventListener("input", cancelSketchCreationReview);
document.getElementById("sketch-new-plane").addEventListener("change", cancelSketchCreationReview);
document.getElementById("sketch-new-cancel").addEventListener("click", cancelSketchCreationReview);
document.getElementById("sketch-new-form").addEventListener("submit", async event => {
  event.preventDefault();
  if (!currentPath || !sketchCreationState || sketchCreateBusy || sketchBusy || featureBusy || documentMutationBusy) return;
  cancelFeatureReview(); clearSketchDraft();
  const sequence = ++sketchCreationSequence, path = currentPath, load = documentLoadSequence;
  const choice = document.getElementById("sketch-new-plane").value;
  const workplane = choice.startsWith("datum:") ? {datum_plane:choice.slice(6)} : {plane:choice};
  const create = {
    document_id: sketchCreationState.document_id, revision: sketchCreationState.revision,
    sketch_id: sketchCreationState.suggested_sketch_id, feature_id: sketchCreationState.suggested_feature_id,
    name: document.getElementById("sketch-new-name").value.trim(),
    ...workplane,
  };
  sketchCreateBusy = true;
  try {
    const plan = await invoke("plan_sketch_create_cmd", {path, create});
    if (sequence !== sketchCreationSequence || path !== currentPath || load !== documentLoadSequence || documentMutationBusy || create.revision !== featureTree?.revision) return;
    reviewedSketchCreation = {path, create: plan.create, load, choice};
    const row = plan.state.sketches.find(row => row.source.id === create.sketch_id);
    const physical = plan.evidence.volume_m3 == null ? "No solid yet" : `Part volume ${(plan.evidence.volume_m3 * 1e9).toFixed(3)} mm³`;
    document.getElementById("sketch-new-effect").textContent = `${create.name} · ${row.workplane_label} · origin ${row.resolved_frame.origin_m.map(v=>(v*1000).toFixed(3)).join(", ")} mm · ${row.solved.solve_state.status} · ${physical}. Source unchanged until Create.`;
    document.getElementById("sketch-new-preview").hidden = false;
  } catch (error) {
    if (sequence === sketchCreationSequence && path === currentPath && load === documentLoadSequence) {
      cancelSketchCreationReview(); setStatus(`Sketch creation refused: ${error}`);
    }
  } finally { sketchCreateBusy = false; }
});
document.getElementById("sketch-new-apply").addEventListener("click", async () => {
  if (sketchCreateBusy || sketchBusy || featureBusy || documentMutationBusy || !reviewedSketchCreation) return;
  const reviewed = reviewedSketchCreation;
  if (reviewed.path !== currentPath || reviewed.load !== documentLoadSequence || reviewed.create.revision !== featureTree?.revision
      || reviewed.create.name !== document.getElementById("sketch-new-name").value.trim()
      || reviewed.choice !== document.getElementById("sketch-new-plane").value) {
    cancelSketchCreationReview(); return;
  }
  sketchCreateBusy = true; documentMutationBusy = true;
  let created = false;
  try {
    const result = await invoke("apply_sketch_create_cmd", {path:reviewed.path, create:reviewed.create, history:documentHistoryState?.history??null});
    created = true;
    acceptDocumentHistory(result.history);
    await loadDocument(reviewed.path, {keepParamHistory:true});
    document.getElementById("sketch-select").value = reviewed.create.sketch_id;
    clearSketchDraft(); drawSketch(); setStatus("Created verified sketch.");
  } catch (error) { setStatus(created ? `Sketch created. Refresh the document view: ${error}` : `Sketch creation refused: ${error}`); }
  finally { sketchCreateBusy = false; documentMutationBusy = false; cancelSketchCreationReview(); }
});
