// Actual native WebKit/WebView2 selection, controls, diagnostics and IPC (ADR-106).
import assert from 'node:assert/strict';
import {cpSync,readFileSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkSelectedConstraints({evaluate,until,source,logicalSource,partPath}) {
  const checks=[],fixture=fileURLToPath(new URL('../examples/datum_plate.ocad.d',import.meta.url));
  const example=JSON.parse(readFileSync(new URL('../examples/agent/constrain_selected_circle.json',import.meta.url),'utf8'));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Previous fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState?.suggested_sketch_id==="sketch:new_1" && !document.getElementById("sketch-new-review").disabled','Constraint fixture loaded');
  const original=logicalSource();
  async function createSketch() {
    const ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
    await evaluate('document.getElementById("sketch-new-name").value="Selected constraints";document.getElementById("sketch-new-plane").value="datum:feature:datum";document.getElementById("sketch-new-form").requestSubmit()');
    await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden','Constraint sketch review');
    await evaluate('document.getElementById("sketch-new-apply").click()');
    await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified sketch."','Constraint sketch creation');
    await evaluate(`document.getElementById("sketch-select").value=${JSON.stringify(ids.sketch)};document.getElementById("sketch-select").dispatchEvent(new Event("change",{bubbles:true}))`);
    return ids;
  }
  // Establish ordinary authored geometry with the real shared backend. Controls
  // under test below always use product DOM events and the unchanged IPC bridge.
  async function seed(operations) {
    await evaluate(`globalThis.__constraintSetup=null;(async()=>{try{const result=await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:{document_id:sketchEditorState.document_id,revision:sketchEditorState.revision,sketch_id:sketchRow().source.id,operations:${JSON.stringify(operations)}},history:documentHistoryState?.history??null});acceptDocumentHistory(result.history);await loadDocument(currentPath,{keepParamHistory:true});globalThis.__constraintSetup="ok";}catch(error){globalThis.__constraintSetup=String(error);}})()`);
    await until('globalThis.__constraintSetup!==null','Real authored fixture setup');assert.equal(await evaluate('globalThis.__constraintSetup'),'ok');
  }
  const ids=await createSketch();await seed(example.primitives);
  await evaluate(`document.getElementById("feature-id").value="feature:disc_body";document.getElementById("feature-name").value="Constrained disc";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value && JSON.parse(o.value).sketch_feature===${JSON.stringify(ids.feature)}).value;document.getElementById("feature-create").requestSubmit()`);
  await until('!featureBusy && !document.getElementById("feature-review").hidden','Initial disc review');
  assert.match(await evaluate('document.getElementById("feature-effect").textContent'),/471\.239 mm³/);
  await evaluate('document.getElementById("feature-apply").click()');
  await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Initial disc apply');
  const before=logicalSource();
  const sketch=()=>logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  async function change(id,value,type='change') {
    await evaluate(`document.getElementById(${JSON.stringify(id)}).value=${JSON.stringify(value)};document.getElementById(${JSON.stringify(id)}).dispatchEvent(new Event(${JSON.stringify(type)},{bubbles:true}))`);
  }
  async function click(id,shift=false) {
    await evaluate(`document.getElementById("sketch-tool").value="select";Array.from(document.querySelectorAll("#sketch-canvas [data-source-id]")).find(e=>e.dataset.sourceId===${JSON.stringify(id)}).dispatchEvent(new MouseEvent("click",{bubbles:true,shiftKey:${shift}}))`);
  }
  async function review(dof) {
    const bytes=source();await evaluate('document.getElementById("sketch-constraint-form").requestSubmit()');
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Selected constraint review');
    assert.equal(source(),bytes);assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),new RegExp(`${dof} DOF`));
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Selected constraint apply');
  }
  async function history(operation) {
    await evaluate(`document.getElementById("${operation}-btn").click()`);
    await until(`!documentMutationBusy && document.getElementById("status").textContent===${JSON.stringify(operation==='undo'?'Undid document edit':'Redid document edit')}`,'Constraint history');
  }
  async function stableState() {
    return {source:source(),history:await evaluate('JSON.stringify(documentHistoryState)'),preview:await evaluate('document.getElementById("preview").src')};
  }
  async function unchanged(saved) {assert.deepEqual(await stableState(),saved);}
  assert.deepEqual(await evaluate('sketchRow().constraint_targets'),{points:['ent:a','ent:b'],lines:['ent:guide'],circles:['ent:disc']});
  assert.match(await evaluate('document.getElementById("sketch-diagnostics").textContent'),/3 DOF/);
  await click('ent:guide');assert.equal(await evaluate('document.getElementById("constraint-target-a").value'),'ent:guide');
  assert.deepEqual(await evaluate('["constraint-b-label","constraint-value-label","constraint-remove-label"].map(id=>getComputedStyle(document.getElementById(id)).display)'),['none','none','none']);
  assert.equal(await evaluate('document.querySelector("#sketch-canvas .sketch-selected").dataset.sourceId'),'ent:guide');
  const saved=await stableState();
  await evaluate('document.getElementById("sketch-constraint-form").requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  await until('!sketchBusy','Canceled real selected-constraint review');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));await unchanged(saved);
  await click('ent:a');await click('ent:b',true);
  assert.deepEqual(await evaluate('[document.getElementById("constraint-kind").value,document.getElementById("constraint-target-a").value,document.getElementById("constraint-target-b").value]'),['distance','ent:a','ent:b']);
  assert.equal(await evaluate('document.querySelectorAll("#sketch-canvas .sketch-selected").length'),2);
  await change('constraint-value','20 mm','input');await review(2);
  await change('constraint-value','21 mm','input');assert(await evaluate('document.getElementById("sketch-review").hidden'));await unchanged(saved);
  checks.push('actual source line/circle/Shift-point selection uses backend target catalog; DOF count, highlights, readonly review, changed-value cancellation and late real IPC');

  await click('ent:guide');await change('constraint-kind','vertical');await review(2);await apply();
  assert.equal(sketch().constraints[0].type,'vertical');
  assert(Math.abs(await evaluate('sketchRow().solved.entities.find(e=>e.id==="ent:b").x')-.002)<1e-8);
  assert.deepEqual(sketch().entities,before['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities);
  await history('undo');assert.deepEqual(logicalSource(),before);
  checks.push('selected vertical guide solves the disposable point coordinate; one Undo restores all canonical authored source');

  await click('ent:guide');await change('constraint-kind','horizontal');await review(2);await apply();
  await click('ent:a');await click('ent:b',true);await change('constraint-value','20 mm','input');await review(1);await apply();
  await click('ent:disc');assert.equal(await evaluate('document.getElementById("constraint-kind").value'),'radius');
  assert.deepEqual(await evaluate('Array.from(document.getElementById("constraint-target-a").options).map(o=>o.value)'),['','ent:disc']);
  await change('constraint-value','10 mm','input');await review(0);
  assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/1884\.956 mm³/);await apply();
  assert.deepEqual(sketch().entities,before['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities);
  assert.deepEqual(sketch().constraints.map(c=>[c.id,c.type]),[['con:control_1','horizontal'],['con:control_2','distance'],['con:control_3','radius']]);
  assert.match(await evaluate('document.getElementById("sketch-diagnostics").textContent'),/Fully constrained · 0 DOF/);
  assert(Math.abs(await evaluate('sketchRow().solved.entities.find(e=>e.id==="ent:b").x')-.022)<1e-8);
  assert.deepEqual(logicalSource()['sketches.json'].sketches.filter(s=>s.id!==ids.sketch),original['sketches.json'].sketches);
  const constrained=logicalSource();
  checks.push('horizontal, point distance and circle radius controls reduce actual DOF 3→2→1→0; native 600*pi mm3 body retains original coordinates, radius and prior source');

  await change('constraint-kind','remove_constraint');await change('constraint-remove','con:control_3');await review(1);
  assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/471\.239 mm³/);await apply();
  const removed=logicalSource();assert.equal(sketch().constraints.length,2);
  await history('undo');assert.deepEqual(logicalSource(),constrained);
  await history('redo');assert.deepEqual(logicalSource(),removed);await history('undo');assert.deepEqual(logicalSource(),constrained);
  const fully=await stableState();
  for(const [target,kind,value,reason] of [['ent:disc','radius','11 mm','contradictory'],['ent:guide','horizontal',null,'redundant constraints']]) {
    await click(target);await change('constraint-kind',kind);if(value)await change('constraint-value',value,'input');
    await evaluate('document.getElementById("sketch-constraint-form").requestSubmit()');
    await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Sketch refused:")','Backend constraint diagnostic');
    assert((await evaluate('document.getElementById("status").textContent')).includes(reason));await unchanged(fully);
  }
  checks.push('authored constraint removal restores radius 5 mm/one DOF; Undo/Redo retains identities; actual contradictory and redundant diagnoses preserve source/history/preview');

  for(const value of ['10','10 deg','0 mm','-1 mm']) {
    await click('ent:disc');await change('constraint-value',value,'input');
    await evaluate('document.getElementById("sketch-constraint-form").requestSubmit()');
    await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Sketch refused:")','Invalid dimensional expression');await unchanged(fully);
  }
  await seed([{operation:'ellipse',id:'ent:unsupported_ellipse',center:'ent:a',major_radius:'2 mm',minor_radius:'1 mm',axis_angle:'0 deg',construction:true,create:true}]);
  await click('ent:unsupported_ellipse');await change('constraint-kind','radius');
  assert.deepEqual(await evaluate('Array.from(document.getElementById("constraint-target-a").options).map(o=>o.value)'),['','ent:disc']);
  const ellipseState=await stableState();
  await evaluate('globalThis.__constraintRefusal=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:{document_id:sketchEditorState.document_id,revision:sketchEditorState.revision,sketch_id:sketchRow().source.id,operations:[{operation:"radius",id:"con:bad",target:"ent:unsupported_ellipse",expr:"2 mm"}]},history:documentHistoryState?.history??null});globalThis.__constraintRefusal="accepted";}catch{globalThis.__constraintRefusal="refused";}})()');
  await until('globalThis.__constraintRefusal!==null','Actual unsupported ellipse constraint refusal');assert.equal(await evaluate('globalThis.__constraintRefusal'),'refused');await unchanged(ellipseState);
  await history('undo');assert.deepEqual(logicalSource(),constrained);
  checks.push('unitless/angle/nonpositive length values fail closed; ellipse radius is absent from target controls and real IPC also refuses it atomically');

  await change('constraint-kind','remove_constraint');await change('constraint-remove','con:control_3');await review(1);
  const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Constraint datum motion');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.deepEqual(await evaluate('sketchConstraintSelection'),[]);
  const moved=await stableState();
  await evaluate(`globalThis.__constraintStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__constraintStale="accepted";}catch{globalThis.__constraintStale="refused";}})()`);
  await until('globalThis.__constraintStale!==null','Actual stale constraint refusal');assert.equal(await evaluate('globalThis.__constraintStale'),'refused');await unchanged(moved);
  await click('ent:disc');await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled && !documentMutationBusy','Reload selected constraint');
  assert.deepEqual(await evaluate('sketchConstraintSelection'),[]);
  const pair=await createSketch();await seed([{operation:'point',id:'ent:free_a',x:'2 mm',y:'-3 mm',create:true},{operation:'point',id:'ent:free_b',x:'26 mm',y:'15 mm',create:true}]);
  const pairBefore=logicalSource();await click('ent:free_a');await click('ent:free_b',true);await change('constraint-kind','coincident');await review(0);await apply();
  const pairAfter=logicalSource(),sourcePair=pairAfter['sketches.json'].sketches.find(s=>s.id===pair.sketch);
  assert.deepEqual(sourcePair.entities,pairBefore['sketches.json'].sketches.find(s=>s.id===pair.sketch).entities);
  assert.equal(sourcePair.constraints[0].type,'coincident');
  assert(Math.abs(await evaluate('sketchRow().solved.entities.find(e=>e.id==="ent:free_b").x')-.002)<1e-8);
  await history('undo');assert.deepEqual(logicalSource(),pairBefore);await history('redo');assert.deepEqual(logicalSource(),pairAfter);
  checks.push('upstream datum/reload cancel selection and review; real stale apply preserves publication; selected free-point coincidence solves to zero DOF with canonical Undo/Redo');
  return checks;
}
