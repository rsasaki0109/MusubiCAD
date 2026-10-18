// ADR-109 actual product controls and real Tauri IPC on both native hosts.
import assert from 'node:assert/strict';
import {cpSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkSelectedCurveEdits({evaluate,until,source,logicalSource,partPath}) {
  const checks=[],fixture=fileURLToPath(new URL('../examples/selected_curve_edit.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Prior native fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  // Inspector evaluation does not await loadDocument's Promise. The previous
  // fixture also owns sketch:new_1, so wait for this source and the completed
  // product load, rather than accepting the previous fixture's revision.
  await until('sketchCreationState && !documentMutationBusy && document.getElementById("status").textContent.startsWith("Loaded ") && featureTree?.revision===sketchEditorState?.revision && sketchEditorState?.sketches.some(s=>s.source.id==="sketch:new_1" && s.source.entities.some(e=>e.id==="ent:left") && s.source.constraints.some(c=>c.id==="con:guide_length"))','Real curve fixture loaded');
  await evaluate('document.getElementById("sketch-select").value="sketch:new_1";document.getElementById("sketch-select").dispatchEvent(new Event("change",{bubbles:true}))');
  const initial=logicalSource(),sketch=()=>logicalSource()['sketches.json'].sketches.find(s=>s.id==='sketch:new_1');
  async function change(id,value) {
    await evaluate(`document.getElementById(${JSON.stringify(id)}).value=${JSON.stringify(value)};document.getElementById(${JSON.stringify(id)}).dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  async function choices(target='ent:guide',cutter='ent:disc',mode='trim',end='end',remove=true) {
    for(const [id,value] of [['sketch-tool','curve_edit'],['curve-target',target],['curve-cutter',cutter],['curve-mode',mode],['curve-end',end]])await change(id,value);
    await evaluate(`for(const c of document.querySelectorAll("#curve-removals input")){c.checked=${remove}&&c.value==="con:guide_length";}document.getElementById("curve-removals").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  async function discover(index='1') {
    const bytes=source();await evaluate('document.getElementById("curve-find").click()');
    await until('!sketchBusy && currentCurveOptions()!==null','Backend intersections');assert.equal(source(),bytes);
    assert.equal(await evaluate('document.getElementById("curve-branch").value'),'');
    assert(await evaluate('document.getElementById("curve-review").disabled'));
    if(index!==null)await change('curve-branch',index);
  }
  async function review(before=1,after=1) {
    const bytes=source();await evaluate('document.getElementById("sketch-curve-form").requestSubmit()');
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Curve review');assert.equal(source(),bytes);
    assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),new RegExp(`Closed profiles: ${before} → ${after}`));
    assert.equal(await evaluate('reviewedSketch.edit.operations.length'),1);
    assert.equal(await evaluate('reviewedSketch.edit.operations[0].operation'),'curve_edit');
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Curve apply');
  }
  async function history(operation) {
    await evaluate(`document.getElementById("${operation}-btn").click()`);
    await until(`!documentMutationBusy && document.getElementById("status").textContent===${JSON.stringify(operation==='undo'?'Undid document edit':'Redid document edit')}`,'Curve history');
  }
  async function stable(){return {source:source(),history:await evaluate('JSON.stringify(documentHistoryState)'),preview:await evaluate('document.getElementById("preview").src')};}
  async function seed(operations) {
    // Fixture additions only: never replace invoke or synthesize a response.
    await evaluate(`globalThis.__curveSetup=null;(async()=>{try{const r=await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:{document_id:sketchEditorState.document_id,revision:sketchEditorState.revision,sketch_id:sketchRow().source.id,operations:${JSON.stringify(operations)}},history:documentHistoryState?.history??null});acceptDocumentHistory(r.history);await loadDocument(currentPath,{keepParamHistory:true});globalThis.__curveSetup="ok";}catch(e){globalThis.__curveSetup=String(e);}})()`);
    await until('globalThis.__curveSetup!==null','Real fixture additions');assert.equal(await evaluate('globalThis.__curveSetup'),'ok');
  }
  async function canvas(id) {
    await evaluate(`Array.from(document.querySelectorAll("#sketch-canvas [data-source-id]")).find(e=>e.dataset.sourceId===${JSON.stringify(id)}).dispatchEvent(new MouseEvent("click",{bubbles:true}))`);
  }
  const catalogue=await evaluate('sketchRow().curve_edit_targets');
  assert(catalogue.targets.includes('ent:guide')&&!catalogue.targets.includes('ent:disc')&&!catalogue.targets.includes('ent:center'));
  await change('sketch-tool','curve_edit');await change('curve-pick-role','target');await canvas('ent:guide');
  await change('curve-pick-role','cutter');await canvas('ent:disc');
  assert.equal(await evaluate('document.getElementById("curve-target").value'),'ent:guide');
  assert.equal(await evaluate('document.getElementById("curve-cutter").value'),'ent:disc');
  const saved=await stable();await discover(null);
  assert.equal(await evaluate('document.querySelectorAll("#sketch-canvas [data-curve-branch]").length'),2);
  assert.match(await evaluate('document.getElementById("curve-branch").textContent'),/10\.000000, 3\.000000.*mm/);
  await evaluate('document.getElementById("curve-find").click();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  await until('!sketchBusy','Late actual discovery discarded');assert.equal(await evaluate('curveOptions'),null);assert.deepEqual(await stable(),saved);
  checks.push('actual canvas target/cutter catalogue and backend-only intersections require an explicit branch; Escape discards late real discovery without source/history/publication');

  await choices('ent:guide','ent:disc','trim','end',false);await discover();
  await evaluate('document.getElementById("sketch-curve-form").requestSubmit()');
  await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Sketch refused:")','Retained length endpoint refusal');
  assert.match(await evaluate('document.getElementById("status").textContent'),/retained constraints/);assert.deepEqual(await stable(),saved);
  await choices();await discover(null);
  await evaluate(`document.querySelector('[data-curve-branch="1"]').dispatchEvent(new KeyboardEvent("keydown",{key:"Enter",bubbles:true}))`);
  await review();assert.deepEqual(await evaluate('reviewedSketch.edit.operations[0].remove_constraints'),['con:guide_length']);
  assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/10\.000000, 3\.000000.*mm/);
  assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/1884\.956 mm³/);
  await apply();const trimmed=logicalSource();
  assert.deepEqual(sketch().constraints,[{type:'horizontal',id:'con:guide_horizontal',line:'ent:guide'}]);
  const originals=initial['sketches.json'].sketches.find(s=>s.id==='sketch:new_1').entities;
  assert.deepEqual(sketch().entities.filter(e=>e.id!=='ent:guide'&&e.id!=='ent:trim_end_1'),originals.filter(e=>e.id!=='ent:guide'));
  assert.equal(sketch().entities.find(e=>e.id==='ent:guide').end,'ent:trim_end_1');
  await history('undo');assert.deepEqual(logicalSource(),initial);await history('redo');assert.deepEqual(logicalSource(),trimmed);
  checks.push('retained line-length constraints refuse the actual solved endpoint; explicit-only removal, keyboard branch review and analytic 600*pi mm3 apply retain old source and canonical Undo/Redo');

  await seed([
    {operation:'point',id:'ent:outer_low',x:'22 mm',y:'-20 mm',create:true},{operation:'point',id:'ent:outer_high',x:'22 mm',y:'20 mm',create:true},
    {operation:'line',id:'ent:outer_cut',start:'ent:outer_low',end:'ent:outer_high',construction:true},
    {operation:'point',id:'ent:axis_low',x:'2 mm',y:'-20 mm',create:true},{operation:'point',id:'ent:axis_high',x:'2 mm',y:'20 mm',create:true},
    {operation:'line',id:'ent:axis_cut',start:'ent:axis_low',end:'ent:axis_high',construction:true},
    {operation:'arc',id:'ent:guide_arc',center_x:'2 mm',center_y:'-3 mm',start_x:'12 mm',start_y:'-3 mm',end_direction_x:'-8 mm',end_direction_y:'-3 mm',clockwise:false,close_sector:false,construction:true}
  ]);
  const beforeExtension=logicalSource();
  await choices('ent:guide','ent:guide_arc','trim','start');await discover('0');await review();await apply();
  const start=sketch().entities.find(e=>e.id===sketch().entities.find(e=>e.id==='ent:guide').start);
  assert(Math.abs(start.x+.006)<1e-9&&Math.abs(start.y-.003)<1e-9);await history('undo');assert.deepEqual(logicalSource(),beforeExtension);
  await choices('ent:guide','ent:outer_cut','extend');await discover('0');await review();await apply();
  let end=sketch().entities.find(e=>e.id===sketch().entities.find(e=>e.id==='ent:guide').end);assert(Math.abs(end.x-.022)<1e-9&&Math.abs(end.y-.003)<1e-9);
  await history('undo');assert.deepEqual(logicalSource(),beforeExtension);
  for(const [mode,y] of [['trim',.007],['extend',-.013]]) {
    await choices('ent:guide_arc','ent:axis_cut',mode);await discover('0');await review();await apply();
    const arc=sketch().entities.find(e=>e.id==='ent:guide_arc'),point=sketch().entities.find(e=>e.id===arc.end_point);
    assert(Math.abs(point.x-.002)<1e-9&&Math.abs(point.y-y)<1e-9);await history('undo');assert.deepEqual(logicalSource(),beforeExtension);
  }
  checks.push('native arc cutter and explicit start/end controls trim a line start, extend its end and trim/extend an arc at independent endpoints while retaining original source through undo');

  await seed([{operation:'rectangle',id:'ent:openable',x1:'40 mm',y1:'40 mm',x2:'60 mm',y2:'50 mm',construction:false},
    {operation:'point',id:'ent:loop_low',x:'50 mm',y:'30 mm',create:true},{operation:'point',id:'ent:loop_high',x:'50 mm',y:'60 mm',create:true},
    {operation:'line',id:'ent:loop_cut',start:'ent:loop_low',end:'ent:loop_high',construction:true}]);
  const edge=sketch().entities.find(e=>e.type==='line'&&e.id.startsWith('ent:openable')&&!e.construction).id;
  await choices(edge,'ent:loop_cut');await discover('0');await review(2,1);await apply();await history('undo');
  const profile=await evaluate(`(()=>{const p=sketchRow().solved.profiles.find(p=>p.entity_ids.includes(${JSON.stringify(edge)}));return p.profile_ref??p.id;})()`);
  await evaluate(`document.getElementById("feature-id").value="feature:loop_body";document.getElementById("feature-name").value="Consumed loop";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value&&JSON.parse(o.value).sketch_feature==="feature:sketch_new_1"&&JSON.parse(o.value).profile_ref===${JSON.stringify(profile)}).value;document.getElementById("feature-create").requestSubmit()`);
  await until('!featureBusy && !document.getElementById("feature-review").hidden','Loop consumer review');await evaluate('document.getElementById("feature-apply").click()');
  await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Loop consumer apply');
  const consumed=await stable();await choices(edge,'ent:loop_cut');await discover('0');await evaluate('document.getElementById("sketch-curve-form").requestSubmit()');
  await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Sketch refused:")','Consumed loop refuses trim');assert.deepEqual(await stable(),consumed);
  await history('undo');
  checks.push('native trim exposes an unconsumed loop opening before apply; an unchanged consumer cannot lose or silently rebind its loop and refusal preserves files/history/publication');

  const cancelState=await stable();
  for(const [id,value] of [['curve-target','ent:guide_arc'],['curve-cutter','ent:axis_cut'],['curve-mode','extend'],['curve-end','start'],['curve-branch',''],['curve-pick-role','target'],['sketch-tool','select'],['sketch-span','140']]) {
    await choices('ent:guide','ent:disc','trim','start');await discover('0');await review(2,2);await change(id,value);
    assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('reviewedSketch'),null);
  }
  await choices('ent:guide','ent:disc','trim','start');await discover('0');
  await evaluate('document.getElementById("sketch-curve-form").requestSubmit();document.getElementById("curve-mode").value="extend";document.getElementById("curve-mode").dispatchEvent(new Event("change",{bubbles:true}))');
  await until('!sketchBusy','Changed mode discards late review');assert(await evaluate('document.getElementById("sketch-review").hidden'));
  await choices('ent:guide','ent:disc','trim','start');await discover('0');await review(2,2);
  await evaluate('document.querySelector("#curve-removals input").checked=true;document.getElementById("curve-removals").dispatchEvent(new Event("change",{bubbles:true}))');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('curveOptions'),null);assert.deepEqual(await stable(),cancelState);
  checks.push('target/cutter/mode/end/branch/removal/pick-role/tool/view changes cancel review; placeholder selection and late real IPC cannot leave an applyable stale patch');

  await choices('ent:guide','ent:disc','trim','start');await discover('0');await review(2,2);const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Curve upstream datum motion');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('curveOptions'),null);const moved=await stable();
  await evaluate(`globalThis.__curveStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__curveStale="accepted";}catch{globalThis.__curveStale="refused";}})()`);
  await until('globalThis.__curveStale!==null','Real stale curve apply');assert.equal(await evaluate('globalThis.__curveStale'),'refused');assert.deepEqual(await stable(),moved);
  await choices('ent:guide','ent:disc','trim','start');await discover('0');await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !documentMutationBusy && !document.getElementById("sketch-new-review").disabled','Curve reload');
  assert.equal(await evaluate('curveOptions'),null);assert.equal(await evaluate('document.getElementById("curve-target").value'),'');
  checks.push('upstream datum motion and reload discard curve discovery/selection; actual stale apply refuses source/history/publication and preserves all original shared point expressions');
  return checks;
}
