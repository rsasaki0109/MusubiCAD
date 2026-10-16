// Actual Tauri WebKit/WebView2 pointer events; real product handlers and IPC.
import assert from 'node:assert/strict';
import {cpSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkPointerRectangles({evaluate,until,source,logicalSource,partPath}) {
  const checks=[];
  const fixture=fileURLToPath(new URL('../examples/datum_plate.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Previous fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState?.suggested_sketch_id==="sketch:new_1" && !document.getElementById("sketch-new-review").disabled','Pointer fixture loaded');
  const original=logicalSource();
  const ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
  await evaluate('document.getElementById("sketch-new-name").value="Pointer rectangles";document.getElementById("sketch-new-plane").value="datum:feature:datum";document.getElementById("sketch-new-form").requestSubmit()');
  await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden','Pointer sketch creation review');
  await evaluate('document.getElementById("sketch-new-apply").click()');
  await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified sketch."','Pointer sketch creation');
  await evaluate('document.getElementById("sketch-span").value="120";document.getElementById("sketch-span").dispatchEvent(new Event("change",{bubbles:true}))');
  const before=logicalSource(),bytes=source(),history=await evaluate('JSON.stringify(documentHistoryState)'),preview=await evaluate('document.getElementById("preview").src');
  async function pointer(x,y,type='click') {
    // WebKit MouseEvent coordinates are integer CSS pixels. Align only the test
    // viewport's origin to that grid; its 120 mm span has 4 pixels/mm. This is
    // presentation setup, not replaced input properties, intent, IPC or geometry.
    // Never call the product's draftPoint/radius helper or manufacture its intent.
    await evaluate(`(()=>{const c=document.getElementById("sketch-canvas");c.style.transform="none";const m=c.getScreenCTM();c.style.transform=\`translate(\${Math.round(m.e)-m.e}px,\${Math.round(m.f)-m.f}px)\`;const p=c.createSVGPoint();p.x=${x};p.y=${-y};const v=p.matrixTransform(c.getScreenCTM());c.dispatchEvent(new ${type==='pointermove'?'PointerEvent':'MouseEvent'}(${JSON.stringify(type)},{bubbles:true,clientX:v.x,clientY:v.y}));})()`);
  }
  async function gesture(construction=false) {
    await evaluate('document.getElementById("sketch-span").value="120";document.getElementById("sketch-span").dispatchEvent(new Event("change",{bubbles:true}))');
    await evaluate(`document.getElementById("sketch-tool").value="rectangle";document.getElementById("sketch-construction").checked=${construction}`);
    await pointer(22,7);await pointer(2,-3,'pointermove');await pointer(2,-3);
  }
  async function review() {
    const stable=source();await evaluate('document.getElementById("sketch-form").requestSubmit()');
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Pointer rectangle review');assert.equal(source(),stable);
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Pointer rectangle apply');
  }
  await evaluate('document.getElementById("sketch-tool").value="rectangle"');
  await pointer(2,-3);await pointer(22,7,'pointermove');
  assert.deepEqual(await evaluate('(()=>{const r=document.getElementById("sketch-draft-rectangle");return ["x","y","width","height"].map(a=>Number(r.getAttribute(a)));})()'),[2,-7,20,10]);
  assert.equal(await evaluate('sketchDraft'),null);
  await evaluate('document.getElementById("sketch-form").requestSubmit()');
  assert.match(await evaluate('document.getElementById("status").textContent'),/^Choose opposite rectangle corner/);
  await evaluate('document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  assert.equal(await evaluate('rectangleStart'),null);assert.equal(await evaluate('!!document.getElementById("sketch-draft-rectangle")'),false);
  await gesture();
  assert.deepEqual(await evaluate('sketchDraft'),[
    {operation:'rectangle',id:'ent:rectangle_1',x1:'22.000 mm',y1:'7.000 mm',x2:'2.000 mm',y2:'-3.000 mm',construction:false},
  ]);
  // Cancel synchronously after starting a real IPC review: its late result must
  // not restore the preview. No response, invoke function or backend is replaced.
  await evaluate('document.getElementById("sketch-form").requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  await until('!sketchBusy','Canceled real review returned');assert(await evaluate('document.getElementById("sketch-review").hidden'));
  await gesture();await evaluate('document.getElementById("sketch-clear").click()');
  assert.equal(source(),bytes);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);assert.equal(await evaluate('document.getElementById("preview").src'),preview);
  checks.push('actual opposite-corner events show a 20x10 mm draft; half review, Escape, discard and late real IPC preserve source/history/preview');
  for(const [x,y] of [[2,7],[22,-3]]) {
    await pointer(2,-3);await pointer(x,y);
    assert.match(await evaluate('document.getElementById("status").textContent'),/^Rectangle refused:/);assert.equal(await evaluate('sketchDraft'),null);
  }
  for(const control of ['sketch-span','sketch-tool','sketch-select']) {
    await evaluate('document.getElementById("sketch-tool").value="rectangle"');await pointer(2,-3);
    await evaluate(`(()=>{const c=document.getElementById(${JSON.stringify(control)});c.value=${control==='sketch-span'?'"140"':control==='sketch-tool'?'"select"':`Array.from(c.options).find(o=>o.value!==${JSON.stringify(ids.sketch)}).value`};c.dispatchEvent(new Event("change",{bubbles:true}));})()`);
    assert.equal(await evaluate('rectangleStart'),null);assert.equal(await evaluate('sketchDraft'),null);
    if(control==='sketch-select')await evaluate(`document.getElementById("sketch-select").value=${JSON.stringify(ids.sketch)};document.getElementById("sketch-select").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  await pointer(2,-3);await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled && !documentMutationBusy','Reload cancels half gesture');
  assert.equal(await evaluate('rectangleStart'),null);assert.equal(source(),bytes);
  checks.push('zero width and height refuse and view/tool/sketch/reload invalidate gestures without partial corners or edges');
  await gesture(true);await review();await apply();
  const construction=logicalSource();let sketch=construction['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.equal(sketch.entities.length,8);
  assert.equal(sketch.entities.filter(e=>e.type==='point'&&!e.construction).length,4);
  assert.equal(sketch.entities.filter(e=>e.type==='line'&&e.construction).length,4);
  assert.equal(sketch.constraints.length,0);
  await evaluate('document.getElementById("undo-btn").click()');await until('!documentMutationBusy && document.getElementById("status").textContent==="Undid document edit"','Undo atomic rectangle');
  assert.deepEqual(logicalSource(),before);
  await evaluate('document.getElementById("redo-btn").click()');await until('!documentMutationBusy && document.getElementById("status").textContent==="Redid document edit"','Redo atomic rectangle');
  assert.deepEqual(logicalSource(),construction);
  checks.push('construction pointer rectangle publishes four points/four edges atomically and one Undo/Redo restores canonical source');
  await gesture();assert.equal(await evaluate('sketchDraft[0].id'),'ent:rectangle_2');await review();await apply();
  await evaluate(`document.getElementById("feature-id").value="feature:pointer_body";document.getElementById("feature-name").value="Pointer rectangular prism";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value && JSON.parse(o.value).sketch_feature===${JSON.stringify(ids.feature)}).value;document.getElementById("feature-create").requestSubmit()`);
  await until('!featureBusy && !document.getElementById("feature-review").hidden','Pointer prism review');
  assert.match(await evaluate('document.getElementById("feature-effect").textContent'),/1200\.000 mm³/);
  await evaluate('document.getElementById("feature-apply").click()');await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Pointer prism apply');
  const physical=logicalSource();sketch=physical['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.deepEqual(sketch.workplane,{type:'datum_plane',feature_id:'feature:datum'});assert.equal(sketch.entities.length,16);
  assert.deepEqual(physical['sketches.json'].sketches.filter(s=>s.id!==ids.sketch),original['sketches.json'].sketches);
  assert.deepEqual(physical['features.json'].feature_nodes.filter(n=>![ids.feature,'feature:pointer_body'].includes(n.id)),original['features.json'].feature_nodes);
  checks.push('pointer IDs avoid existing geometry; nonconstruction rectangle yields native 1200 mm3 prism and preserves datum association/unrelated source');
  await gesture(true);await review();const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Pointer datum motion');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('sketchDraft'),null);
  const stable=source(),saved=await evaluate('JSON.stringify(documentHistoryState)'),published=await evaluate('document.getElementById("preview").src');
  await evaluate(`globalThis.__pointerRectangleStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__pointerRectangleStale="accepted";}catch{globalThis.__pointerRectangleStale="refused";}})()`);
  await until('globalThis.__pointerRectangleStale!==null','Real stale pointer refusal');assert.equal(await evaluate('globalThis.__pointerRectangleStale'),'refused');
  assert.equal(source(),stable);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),saved);assert.equal(await evaluate('document.getElementById("preview").src'),published);
  assert.equal(logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities.length,16);
  checks.push('upstream datum edit cancels pointer review and actual stale command cannot publish a partial rectangle or alter history/preview');
  return checks;
}
