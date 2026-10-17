// Actual native WebKit/WebView2 DOM controls and unchanged real IPC (ADR-107).
import assert from 'node:assert/strict';
import {cpSync,readFileSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkSelectedConstruction({evaluate,until,source,logicalSource,partPath}) {
  const checks=[],fixture=fileURLToPath(new URL('../examples/datum_plate.ocad.d',import.meta.url));
  const example=JSON.parse(readFileSync(new URL('../examples/agent/toggle_selected_construction.json',import.meta.url),'utf8'));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Prior native fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState?.suggested_sketch_id==="sketch:new_1" && !document.getElementById("sketch-new-review").disabled','Construction fixture loaded');
  const original=logicalSource(),ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
  await evaluate('document.getElementById("sketch-new-name").value="Selected construction";document.getElementById("sketch-new-plane").value="datum:feature:datum";document.getElementById("sketch-new-form").requestSubmit()');
  await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden','Construction sketch review');
  await evaluate('document.getElementById("sketch-new-apply").click()');
  await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified sketch."','Construction sketch apply');
  await evaluate(`document.getElementById("sketch-select").value=${JSON.stringify(ids.sketch)};document.getElementById("sketch-select").dispatchEvent(new Event("change",{bubbles:true}))`);
  async function seed(operations) {
    await evaluate(`globalThis.__constructionSetup=null;(async()=>{try{const result=await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:{document_id:sketchEditorState.document_id,revision:sketchEditorState.revision,sketch_id:sketchRow().source.id,operations:${JSON.stringify(operations)}},history:documentHistoryState?.history??null});acceptDocumentHistory(result.history);await loadDocument(currentPath,{keepParamHistory:true});globalThis.__constructionSetup="ok";}catch(error){globalThis.__constructionSetup=String(error);}})()`);
    await until('globalThis.__constructionSetup!==null','Real source setup');assert.equal(await evaluate('globalThis.__constructionSetup'),'ok');
  }
  // Setup ordinary admitted source via the real gateway; all flag controls
  // under test below are exercised through product DOM events, not injected IPC.
  await seed(example.primitives);const initial=logicalSource();
  const sketch=()=>logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  async function click(id) {
    await evaluate(`document.getElementById("sketch-tool").value="select";Array.from(document.querySelectorAll("#sketch-canvas [data-source-id]")).find(e=>e.dataset.sourceId===${JSON.stringify(id)}).dispatchEvent(new MouseEvent("click",{bubbles:true}))`);
    assert.equal(await evaluate('document.getElementById("construction-target").value'),id);
  }
  async function target(id) {
    await evaluate(`document.getElementById("construction-target").value=${JSON.stringify(id)};document.getElementById("construction-target").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  async function review(before,after) {
    const bytes=source();await evaluate('document.getElementById("sketch-construction-form").requestSubmit()');
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Flag-only review');
    assert.equal(source(),bytes);
    assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),new RegExp(`Closed profiles: ${before} → ${after}`));
    const edit=await evaluate('reviewedSketch.edit');assert.equal(edit.operations.length,1);assert.equal(edit.operations[0].operation,'construction');
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Flag-only apply');
  }
  async function history(operation) {
    await evaluate(`document.getElementById("${operation}-btn").click()`);
    await until(`!documentMutationBusy && document.getElementById("status").textContent===${JSON.stringify(operation==='undo'?'Undid document edit':'Redid document edit')}`,'Flag history');
  }
  async function stable() {return {source:source(),history:await evaluate('JSON.stringify(documentHistoryState)'),preview:await evaluate('document.getElementById("preview").src')};}
  const targets=await evaluate('sketchRow().construction_targets');
  assert.deepEqual(targets.map(t=>[t.id,t.kind,t.construction]),[['ent:a','point',false],['ent:b','point',false],['ent:guide','line',true],['ent:disc','circle',true],['ent:ellipse','ellipse',true]]);
  assert(!await evaluate('sketchRow().constraint_targets.circles.includes("ent:ellipse")'));
  await click('ent:disc');assert.equal(await evaluate('document.getElementById("construction-review").textContent'),'Review make ordinary');
  assert.match(await evaluate('document.getElementById("construction-info").textContent'),/ent:disc is construction/);
  const saved=await stable();
  await evaluate('document.getElementById("sketch-construction-form").requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  await until('!sketchBusy','Late real construction review');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.deepEqual(await stable(),saved);
  await click('ent:disc');await review(0,1);assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/ent:disc/);
  checks.push('backend construction catalogue distinguishes ellipse eligibility from constraint radius; canvas current-state selection and flag-only read-only review reject late real IPC after Escape');

  await apply();const ordinary=logicalSource();
  const expected=structuredClone(initial);expected['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities.find(e=>e.id==='ent:disc').construction=false;
  // Derived profiles/solve_state may refresh; all authored geometry must agree.
  assert.deepEqual(sketch().entities,expected['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities);
  assert.equal(sketch().entities.find(e=>e.id==='ent:disc').radius,'1 cm');
  await history('undo');assert.deepEqual(logicalSource(),initial);await history('redo');assert.deepEqual(logicalSource(),ordinary);
  await evaluate(`document.getElementById("feature-id").value="feature:disc_body";document.getElementById("feature-name").value="Ordinary disc";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value && JSON.parse(o.value).sketch_feature===${JSON.stringify(ids.feature)}).value;document.getElementById("feature-create").requestSubmit()`);
  await until('!featureBusy && !document.getElementById("feature-review").hidden','Converted circle extrusion review');
  assert.match(await evaluate('document.getElementById("feature-effect").textContent'),/1884\.956 mm³/);
  await evaluate('document.getElementById("feature-apply").click()');
  await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Converted circle extrusion apply');
  checks.push('new make-ordinary control preserves literal 1 cm source expression and IDs; canonical Undo/Redo then actual native extrusion yields 600*pi mm3');

  await click('ent:ellipse');assert.equal(await evaluate('document.getElementById("construction-review").textContent'),'Review make ordinary');
  assert.equal(await evaluate('document.querySelector("#sketch-canvas .sketch-selected").dataset.sourceId'),'ent:ellipse');
  await review(1,2);assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/feature:disc_body → profile:circle:ent:disc.*existing loop bindings retained/);
  assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/1884\.956 mm³/);await apply();
  assert.equal(sketch().entities.find(e=>e.id==='ent:ellipse').major_radius,'15 mm + 15 mm');
  const withEllipse=logicalSource();await history('undo');await history('redo');assert.deepEqual(logicalSource(),withEllipse);
  const consumed=await stable();await click('ent:disc');await evaluate('document.getElementById("sketch-construction-form").requestSubmit()');
  await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Sketch refused:")','Consumed circle construction refusal');
  assert.match(await evaluate('document.getElementById("status").textContent'),/profile/);assert.deepEqual(await stable(),consumed);
  checks.push('selected ellipse exposes true closed-loop 1→2 consequences without literalizing axes; unchanged consumer/solid are shown and consumed-circle removal refuses source/history/publication atomically');

  await click('ent:a');assert.match(await evaluate('document.getElementById("construction-info").textContent'),/point flag affects this point only; curves using it remain eligible/);
  await review(2,2);await apply();assert.equal(sketch().entities.find(e=>e.id==='ent:a').construction,true);
  await click('ent:a');assert.equal(await evaluate('document.getElementById("construction-review").textContent'),'Review make ordinary');
  assert(await evaluate('Array.from(document.querySelectorAll("#sketch-canvas .sketch-point.construction")).some(e=>e.dataset.sourceId==="ent:a")'));
  await click('ent:guide');await review(2,2);await apply();assert.equal(sketch().entities.find(e=>e.id==='ent:guide').construction,false);
  await history('undo');const beforeArc=logicalSource();
  await seed([{operation:'arc',id:'ent:guide_arc',center_x:'-30 mm',center_y:'0 mm',start_x:'-20 mm',start_y:'0 mm',end_direction_x:'-30 mm',end_direction_y:'10 mm',clockwise:false,close_sector:true,construction:true}]);
  const arcSource=sketch().entities;await click('ent:guide_arc');await review(2,2);await apply();
  assert.deepEqual(sketch().entities,arcSource.map(e=>e.id==='ent:guide_arc'?{...e,construction:false}:e));
  await history('undo');await history('undo');assert.deepEqual(logicalSource(),beforeArc);
  checks.push('point flags retain the consumed circle and point-only presentation; selected line/arc flags preserve exact source and expose open-curve profile consequences with history');

  await click('ent:b');await review(2,2);await target('ent:ellipse');assert(await evaluate('document.getElementById("sketch-review").hidden'));
  const cancelState=await stable();
  await evaluate('document.getElementById("sketch-construction-form").requestSubmit();document.getElementById("construction-target").value="ent:b";document.getElementById("construction-target").dispatchEvent(new Event("change",{bubbles:true}))');
  await until('!sketchBusy','Changed source discards late real review');assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.deepEqual(await stable(),cancelState);
  await click('ent:ellipse');await evaluate('document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  assert.equal(await evaluate('document.getElementById("construction-target").value'),'');
  for(const [id,value] of [['sketch-tool','line'],['sketch-span','140']]) {
    await target('ent:b');await review(2,2);
    await evaluate(`document.getElementById(${JSON.stringify(id)}).value=${JSON.stringify(value)};document.getElementById(${JSON.stringify(id)}).dispatchEvent(new Event("change",{bubbles:true}))`);
    assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('document.getElementById("construction-target").value'),'');
  }
  assert.deepEqual(await stable(),cancelState);
  checks.push('target/tool/span changes cancel construction reviews and selection; ellipse-only Escape and changed-target late real IPC preserve source/history/preview');

  await target('ent:b');await review(2,2);const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Upstream construction datum move');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('document.getElementById("construction-target").value'),'');
  const moved=await stable();
  await evaluate(`globalThis.__constructionStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__constructionStale="accepted";}catch{globalThis.__constructionStale="refused";}})()`);
  await until('globalThis.__constructionStale!==null','Actual stale construction apply');assert.equal(await evaluate('globalThis.__constructionStale'),'refused');assert.deepEqual(await stable(),moved);
  await target('ent:ellipse');await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled && !documentMutationBusy','Construction reload');
  assert.equal(await evaluate('document.getElementById("construction-target").value'),'');
  assert.deepEqual(logicalSource()['sketches.json'].sketches.filter(s=>s.id!==ids.sketch),original['sketches.json'].sketches);
  checks.push('upstream datum edits and reload discard source selection/review; real stale IPC refuses without publishing or changing opaque history and unrelated sketches remain intact');
  return checks;
}
