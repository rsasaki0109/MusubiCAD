// ADR-111. Disposable form state; source, choices and verification stay backend-owned.
const createHoleControls = ({invoke, context, clearOtherDrafts, acceptHistory, reload, setBusy, setStatus}) => {
  const control = id => document.getElementById("hole-" + id);
  let state = null, reviewed = null, sequence = 0, working = false;
  const axisKey = c => JSON.stringify([c.sketch_feature, c.profile_ref]);
  const choices = () => (state?.choices ?? []).filter(c => c.target_feature === control("stock").value && c.face_ref === control("entry").value);
  const choice = () => choices().find(c => axisKey(c) === control("axis").value);
  const mm = value => (value * 1000).toFixed(6);
  const point = values => values.map(mm).join(", ");
  const standardLabel = s => s.type === "iso273" ? `ISO 273 ${s.size} ${s.fit}` : `${s.size} coarse tap-drill cylinder`;
  function cancel() { sequence++; reviewed = null; control("preview").hidden = true; }
  function fill(id, items) {
    const select = control(id), old = select.value; select.replaceChildren();
    const seen = new Set();
    for (const [value, label] of items) {
      if (seen.has(value)) continue; seen.add(value);
      const option = document.createElement("option"); option.value = value; option.textContent = label; select.append(option);
    }
    if (seen.has(old)) select.value = old;
  }
  function showAxis() {
    const c = choice();
    fill("standard", (c?.standards ?? []).map(s => [JSON.stringify(s), standardLabel(s)]));
    control("info").textContent = c ? `Bore diameter ${mm(c.bore_diameter_m)} mm; entry centre (${point(c.entry_center_m)}) mm, inward direction (${c.inward_direction.map(v => v.toFixed(6)).join(", ")}); stock thickness ${mm(c.stock_thickness_m)} mm.`
      : "Choose original rectangular stock and an existing circle on its owned end face. Modified stock, side entries and unsupported axes are unavailable.";
    control("review").disabled = !c;
  }
  function showEntries() {
    fill("entry", (state?.choices ?? []).filter(c => c.target_feature === control("stock").value).map(c => [c.face_ref, `${c.entry_role} · ${c.face_ref}`]));
    showAxes();
  }
  function showAxes() { fill("axis", choices().map(c => [axisKey(c), `${c.axis_name} · ${c.circle_id}`])); showAxis(); }
  function render(next) {
    cancel(); state = next;
    fill("stock", (state?.choices ?? []).map(c => [c.target_feature, c.stock_name])); showEntries();
  }
  function seatFields() {
    const kind = control("seat").value;
    control("diameter-label").hidden = kind === "none";
    control("seat-depth-label").hidden = kind !== "counterbore";
    control("angle-label").hidden = kind !== "countersink";
  }
  function changed() { clearOtherDrafts(); cancel(); }
  for (const id of ["stock", "entry", "axis", "standard", "seat"]) control(id).addEventListener("change", () => {
    changed(); if (id === "stock") showEntries(); else if (id === "entry") showAxes(); else if (id === "axis") showAxis();
    seatFields();
  });
  for (const id of ["name", "depth", "diameter", "seat-depth", "angle"]) control(id).addEventListener("input", changed);
  control("cancel").addEventListener("click", cancel);
  document.addEventListener("keydown", event => { if (event.key === "Escape") cancel(); });
  control("form").addEventListener("submit", async event => {
    event.preventDefault(); const current = context(), c = choice();
    if (working || current.busy || !current.path || !state || !c || state.revision !== current.revision) return;
    const type = control("seat").value;
    const seat = type === "counterbore" ? {type, diameter: control("diameter").value.trim(), depth: control("seat-depth").value.trim()}
      : type === "countersink" ? {type, diameter: control("diameter").value.trim(), included_angle: control("angle").value.trim()} : {type: "none"};
    const create = {document_id: state.document_id, revision: state.revision, target_feature: c.target_feature, face_ref: c.face_ref,
      sketch_feature: c.sketch_feature, profile_ref: c.profile_ref, standard: JSON.parse(control("standard").value),
      name: control("name").value.trim(), depth: control("depth").value.trim(), seat};
    clearOtherDrafts(); cancel(); const seq = sequence; working = true;
    setStatus("Verifying hole…");
    try {
      const plan = await invoke("plan_hole_create_cmd", {path: current.path, create});
      const now = context();
      if (seq !== sequence || now.path !== current.path || now.loadSequence !== current.loadSequence || now.revision !== create.revision || now.busy) return;
      reviewed = {path: current.path, create: plan.create};
      const c = plan.choice, seat = plan.create.seat;
      const seatText = seat.type === "none" ? "Plain bore" : `${seat.type === "counterbore" ? "Counterbore" : "Countersink"}: diameter ${mm(plan.seat_diameter_m)} mm, depth ${mm(plan.seat_depth_m)} mm${seat.type === "countersink" ? `, included angle ${seat.included_angle}` : ""}`;
      control("effect").textContent = `${plan.create.name}: ${standardLabel(plan.create.standard)}, bore ${mm(c.bore_diameter_m)} mm × ${mm(plan.bore_depth_m)} mm inward from ${c.entry_role}. ${seatText}. Centre (${point(c.entry_center_m)}) mm; inward (${c.inward_direction.map(v => v.toFixed(6)).join(", ")}). Removed ${(plan.removed_volume_m3 * 1e9).toFixed(6)} mm³; remaining ${(plan.evidence.volume_m3 * 1e9).toFixed(6)} mm³. Geometry and required checks passed. Original stock and circle remain unchanged. Source changes only when Create is pressed.`;
      control("preview").hidden = false; setStatus("Hole verified. Review before creating.");
    } catch (error) { if (seq === sequence) { cancel(); setStatus(`Hole refused: ${error}`); } }
    finally { working = false; }
  });
  control("apply").addEventListener("click", async () => {
    const current = context(), draft = reviewed;
    if (working || current.busy || !draft) return;
    if (draft.path !== current.path || draft.create.revision !== current.revision) { cancel(); return; }
    working = true; setBusy(true);
    try {
      const result = await invoke("apply_hole_create_cmd", {path: draft.path, create: draft.create, history: current.history});
      acceptHistory(result.history); await reload(draft.path, {keepParamHistory: true}); setStatus("Created verified hole.");
    } catch (error) { setStatus(`Hole refused: ${error}`); }
    finally { working = false; setBusy(false); cancel(); }
  });
  seatFields();
  return {render, cancel, busy: () => working};
};
