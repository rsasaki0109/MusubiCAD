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
  const relatedNames = focusRelatedParameters(
    relatedParameterIds(summary.selection, summary),
  );
  let message =
    sourceLabel === "preview" ? `Selected ${label}` : `3D viewport: ${label}`;
  if (relatedNames.length) {
    message += ` — related: ${relatedNames.join(", ")}`;
  }
  setStatus(message);
}

async function pickAtPreview(event) {
  if (!currentPath) {
    return;
  }
  const coords = previewImageCoords(event);
  if (!coords) {
    return;
  }

  setStatus(`Picking at ${coords.x.toFixed(0)}, ${coords.y.toFixed(0)}…`);
  try {
    const summary = await invoke("pick_document_cmd", {
      path: currentPath,
      x: coords.x,
      y: coords.y,
    });
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
  const state = await invoke("set_document_parameter_cmd", {
    path: currentPath,
    id,
    expr,
    history: documentHistoryState?.history ?? null,
  });
  acceptDocumentHistory(state);
  await loadDocument(currentPath, { keepParamHistory: true });
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
  if (!currentPath || !documentHistoryState?.can_undo) {
    return;
  }

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
  }
}

async function redoParameterChange() {
  if (!currentPath || !documentHistoryState?.can_redo) {
    return;
  }

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
  }
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
  if (!path) {
    setStatus("No document selected.");
    return;
  }

  currentPath = path;
  if (!options.keepParamHistory) {
    resetDocumentHistory();
  }
  setStatus(`Loading ${path}…`);

  // A document that no longer regenerates has no preview, but its info,
  // parameters, and regeneration inspection still load so it can be repaired.
  const [inspectResult, previewResult, regenerationResult, parametersResult] =
    await Promise.allSettled([
      invoke("inspect_document_cmd", { path }),
      invoke("preview_document_cmd", { path }),
      invoke("inspect_regeneration_cmd", { path }),
      options.skipParameters
        ? Promise.resolve(null)
        : invoke("list_document_parameters_cmd", { path }),
    ]);
  if (inspectResult.status === "rejected") {
    throw inspectResult.reason;
  }
  const inspect = inspectResult.value;

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
    preview.removeAttribute("src");
    preview.alt = "No preview";
    renderInfo(previewInfo, [["Preview", "unavailable"]]);
    setStatus(`Loaded ${inspect.name} without a preview: ${previewResult.reason}`);
    return;
  }
  const previewData = previewResult.value;
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
