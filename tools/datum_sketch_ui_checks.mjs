// Shared actual Linux WebKit / Windows WebView2 source-bound datum form checks.
import assert from 'node:assert/strict';
import {cpSync, realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkDatumSketches({evaluate,until,source,logicalSource,partPath}) {
  const checks=[];
  // Establish a second disposable source fixture, then exercise real product
  // controls. No frontend graph/solver, IPC response or backend result is faked.
  const fixture=fileURLToPath(new URL('../examples/datum_plate.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Previous fixture idle');
  cpSync(fixture,partPath,{recursive:true});
  await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState?.datum_planes.some(p=>p.feature_id==="feature:datum") && !document.getElementById("sketch-new-review").disabled','Actual datum choices loaded');
  const original=logicalSource(),originalBytes=source(),initialHistory=await evaluate('JSON.stringify(documentHistoryState)');
  const ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
  async function creationReview() {
    await evaluate('document.getElementById("sketch-new-name").value="Datum manufacturing sketch";document.getElementById("sketch-new-plane").value="datum:feature:datum";document.getElementById("sketch-new-form").requestSubmit()');
    await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden','Datum creation review');
  }
  await creationReview();assert.equal(source(),originalBytes);
  assert.match(await evaluate('document.getElementById("sketch-new-effect").textContent'),/Datum · datum · origin 0\.000, 0\.000, 5\.000 mm/);
  await evaluate('document.getElementById("sketch-new-cancel").click()');
  assert.equal(source(),originalBytes);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),initialHistory);
  checks.push('native datum choice uses regenerated labelled frame; review/cancel does not write source/history');
  await creationReview();await evaluate('document.getElementById("sketch-new-apply").click()');
  await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified sketch."','Datum creation apply');
  assert.equal(await evaluate('document.getElementById("sketch-select").value'),ids.sketch);
  let actual=logicalSource();
  assert.deepEqual(actual['sketches.json'].sketches.find(s=>s.id===ids.sketch).workplane,{type:'datum_plane',feature_id:'feature:datum'});
  assert(actual['features.json'].feature_graph.edges.some(e=>e.source==='feature:datum' && e.target===ids.feature));
  async function sketchReview(action,create) {
    const before=source();
    await evaluate(`document.getElementById("sketch-action").value=${JSON.stringify(action)};document.getElementById("sketch-id").value=${JSON.stringify(action==='ellipse'?'ent:new_ellipse':'ent:new_center')};document.getElementById("sketch-a").value="ent:new_center";document.getElementById("sketch-x").value=${JSON.stringify(action==='ellipse'?'20 mm':create?'0 mm':'2 mm')};document.getElementById("sketch-y").value=${JSON.stringify(action==='ellipse'?'10 mm':create?'0 mm':'-3 mm')};document.getElementById("sketch-angle").value="0 deg";document.getElementById("sketch-create").checked=${create};document.getElementById("sketch-construction").checked=false;document.getElementById("sketch-form").requestSubmit()`);
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Local datum edit review');
    assert.equal(source(),before);
  }
  async function sketchApply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Local datum edit apply');
  }
  await sketchReview('point',true);await sketchApply();
  await sketchReview('ellipse',true);await sketchApply();
  checks.push('native creation atomically preserves datum dependency and supports actual local point/ellipse editing');
  await evaluate(`document.getElementById("feature-id").value="feature:ui_datum_body";document.getElementById("feature-name").value="Datum ellipse prism";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value && JSON.parse(o.value).sketch_feature===${JSON.stringify(ids.feature)}).value;document.getElementById("feature-create").requestSubmit()`);
  await until('!featureBusy && !document.getElementById("feature-review").hidden','Review actual datum prism');
  assert.match(await evaluate('document.getElementById("feature-effect").textContent'),/3769\.911 mm³/);
  await evaluate('document.getElementById("feature-apply").click()');
  await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Apply actual datum prism');
  checks.push('native datum sketch produces the independently specified 1200*pi mm3 ellipse prism');
  const beforeParameter=logicalSource();
  await sketchReview('point',false);
  const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Move source datum through actual parameter control');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));
  const frame=await evaluate(`sketchEditorState.sketches.find(r=>r.source.id===${JSON.stringify(ids.sketch)}).resolved_frame`);
  assert(Math.abs(frame.origin_m[2]-.015)<1e-12);
  const afterParameter=logicalSource(),stable=source(),history=await evaluate('JSON.stringify(documentHistoryState)'),preview=await evaluate('document.getElementById("preview").src');
  await evaluate(`globalThis.__datumStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__datumStale="accepted";}catch{globalThis.__datumStale="refused";}})()`);
  await until('globalThis.__datumStale!==null','Actual stale datum edit refusal');
  assert.equal(await evaluate('globalThis.__datumStale'),'refused');assert.equal(source(),stable);
  assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);assert.equal(await evaluate('document.getElementById("preview").src'),preview);
  checks.push('actual datum parameter control moves regenerated frame and invalidates/refuses stale review without writes');
  await evaluate('document.getElementById("undo-btn").click()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Undid document edit"','Undo source datum motion');
  assert.deepEqual(logicalSource(),beforeParameter);
  await evaluate('document.getElementById("redo-btn").click()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Redid document edit"','Redo source datum motion');
  assert.deepEqual(logicalSource(),afterParameter);
  await sketchReview('point',false);await sketchApply();actual=logicalSource();
  const created=actual['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.deepEqual(created.workplane,{type:'datum_plane',feature_id:'feature:datum'});
  assert.deepEqual(created.entities.find(e=>e.id==='ent:new_center'),{type:'point',id:'ent:new_center',construction:false,x:'2 mm',y:'-3 mm'});
  assert.deepEqual(actual['sketches.json'].sketches.filter(s=>s.id!==ids.sketch),original['sketches.json'].sketches);
  assert.deepEqual(actual['features.json'].feature_nodes.filter(n=>![ids.feature,'feature:ui_datum_body'].includes(n.id)),original['features.json'].feature_nodes);
  checks.push('native parameter undo/redo and subsequent local edits preserve authored datum source and prior geometry');
  return checks;
}
