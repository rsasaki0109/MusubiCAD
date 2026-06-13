const { invoke } = window.__TAURI__.core;
const { open, save } = window.__TAURI__.dialog;

const preview = document.getElementById("preview");
const status = document.getElementById("status");
const docInfo = document.getElementById("doc-info");
const previewInfo = document.getElementById("preview-info");
const templateSelect = document.getElementById("template-select");
const openBtn = document.getElementById("open-btn");
const refreshBtn = document.getElementById("refresh-btn");
const createBtn = document.getElementById("create-btn");

let currentPath = null;

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

async function loadDocument(path) {
  if (!path) {
    setStatus("No document selected.");
    return;
  }

  currentPath = path;
  setStatus(`Regenerating ${path}…`);

  const [inspect, previewData] = await Promise.all([
    invoke("inspect_document_cmd", { path }),
    invoke("preview_document_cmd", { path }),
  ]);

  preview.src = `data:image/png;base64,${previewData.png_base64}`;
  preview.alt = previewData.name;

  renderInfo(docInfo, [
    ["Name", inspect.name],
    ["ID", inspect.id],
    ["Path", path],
    ["Sketches", inspect.sketches],
    ["Features", inspect.features],
    ["Parameters", inspect.parameters],
    ["Topo refs", inspect.semantic_refs],
  ]);

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

async function pickDocument() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open .ocad.d directory",
  });
  if (selected) {
    await loadDocument(selected);
  }
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

async function boot() {
  try {
    await loadTemplates();
    const defaultPath = await invoke("default_example_path");
    if (defaultPath) {
      await loadDocument(defaultPath);
    } else {
      setStatus("Open a .ocad.d directory to preview.");
    }
  } catch (error) {
    setStatus(`Error: ${error}`);
  }
}

openBtn.addEventListener("click", () => {
  pickDocument().catch((error) => setStatus(`Error: ${error}`));
});

refreshBtn.addEventListener("click", () => {
  loadDocument(currentPath).catch((error) => setStatus(`Error: ${error}`));
});

createBtn.addEventListener("click", () => {
  createSample().catch((error) => setStatus(`Error: ${error}`));
});

boot();
