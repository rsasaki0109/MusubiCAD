// Actual native WebKit/WebView2 events, product handlers and IPC (ADR-104).
import assert from 'node:assert/strict';
import {cpSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkPointerArcs({evaluate,until,source,logicalSource,partPath}) {
  const checks=[],fixture=fileURLToPath(new URL('../examples/datum_plate.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Previous fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState?.suggested_sketch_id==="sketch:new_1" && !document.getElementById("sketch-new-review").disabled','Arc fixture loaded');
  const original=logicalSource(),ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
  await evaluate('document.getElementById("sketch-new-name").value="Pointer arcs";document.getElementById("sketch-new-plane").value="datum:feature:datum";document.getElementById("sketch-new-form").requestSubmit()');
  await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden','Arc sketch review');
  await evaluate('document.getElementById("sketch-new-apply").click()');
  await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified sketch."','Arc sketch creation');
  const before=logicalSource(),bytes=source(),history=await evaluate('JSON.stringify(documentHistoryState)'),preview=await evaluate('document.getElementById("preview").src');
  async function pointer(x,y,type='click') {
    // Align presentation only for integer WebKit MouseEvent CSS coordinates.
    // No input property, product math, typed intent or IPC result is replaced.
    await evaluate(`(()=>{const c=document.getElementById("sketch-canvas");c.style.transform="none";const m=c.getScreenCTM();c.style.transform=\`translate(\${Math.round(m.e)-m.e}px,\${Math.round(m.f)-m.f}px)\`;const p=c.createSVGPoint();p.x=${x};p.y=${-y};const v=p.matrixTransform(c.getScreenCTM());c.dispatchEvent(new ${type==='pointermove'?'PointerEvent':'MouseEvent'}(${JSON.stringify(type)},{bubbles:true,clientX:v.x,clientY:v.y}));})()`);
  }
  async function configure(clockwise=false,closed=true,construction=false) {
    await evaluate(`document.getElementById("sketch-span").value="120";document.getElementById("sketch-tool").value="arc";document.getElementById("sketch-arc-direction").value=${JSON.stringify(clockwise?'cw':'ccw')};document.getElementById("sketch-arc-sector").checked=${closed};document.getElementById("sketch-construction").checked=${construction};document.getElementById("sketch-tool").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  async function gesture(clockwise=false,closed=true,construction=false) {
    await configure(clockwise,closed,construction);await pointer(2,-3);await pointer(12,-3);await pointer(2,17,'pointermove');await pointer(2,17);
  }
  async function review() {
    const stable=source();await evaluate('document.getElementById("sketch-form").requestSubmit()');
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Arc review');assert.equal(source(),stable);
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Arc apply');
  }
  async function undo() {
    await evaluate('document.getElementById("undo-btn").click()');
    await until('!documentMutationBusy && document.getElementById("status").textContent==="Undid document edit"','Arc Undo');
  }
  async function body(id,volume) {
    await evaluate(`document.getElementById("feature-id").value=${JSON.stringify(id)};document.getElementById("feature-name").value="Pointer sector";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value && JSON.parse(o.value).sketch_feature===${JSON.stringify(ids.feature)}).value;document.getElementById("feature-create").requestSubmit()`);
    await until('!featureBusy && !document.getElementById("feature-review").hidden','Arc sector solid review');
    assert((await evaluate('document.getElementById("feature-effect").textContent')).includes(`${volume} mm³`));
    await evaluate('document.getElementById("feature-apply").click()');
    await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Arc sector solid apply');
  }
  await configure();await pointer(2,-3);await pointer(12,-3);await pointer(2,17,'pointermove');
  const path=await evaluate('document.querySelector("#sketch-draft-arc path").getAttribute("d").split(" ")');
  assert.deepEqual(path.slice(0,9),['M','12','3','A','10','10','0','0','0']);
  assert(Math.abs(Number(path[9])-2)<1e-12 && Number(path[10])===-7);
  assert.equal(await evaluate('sketchDraft'),null);
  await evaluate('document.getElementById("sketch-form").requestSubmit()');
  assert.match(await evaluate('document.getElementById("status").textContent'),/^Finish arc/);
  await evaluate('document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  assert.equal(await evaluate('arcCenter'),null);assert.equal(await evaluate('arcStart'),null);assert.equal(await evaluate('!!document.getElementById("sketch-draft-arc")'),false);
  await gesture();
  assert.deepEqual(await evaluate('sketchDraft'),[{operation:'arc',id:'ent:arc_1',center_x:'2.000 mm',center_y:'-3.000 mm',start_x:'12.000 mm',start_y:'-3.000 mm',end_direction_x:'2.000 mm',end_direction_y:'17.000 mm',clockwise:false,close_sector:true,construction:false}]);
  await evaluate('document.getElementById("sketch-form").requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  await until('!sketchBusy','Canceled real arc review');assert(await evaluate('document.getElementById("sketch-review").hidden'));
  await gesture();await evaluate('document.getElementById("sketch-clear").click()');
  await evaluate('document.getElementById("sketch-form").requestSubmit()');
  assert.match(await evaluate('document.getElementById("status").textContent'),/^Finish arc/);
  assert.equal(source(),bytes);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);assert.equal(await evaluate('document.getElementById("preview").src'),preview);
  checks.push('actual centre/start/end-direction events keep radius 10 mm; half review, Escape, discard and late real IPC preserve source/history/preview');

  for(const failure of ['radius','direction','full']) {
    await configure();await pointer(2,-3);
    if(failure==='radius')await pointer(2,-3);
    else {await pointer(12,-3);await pointer(failure==='full'?22:2,-3);}
    assert.match(await evaluate('document.getElementById("status").textContent'),/^Arc refused:/);assert.equal(await evaluate('sketchDraft'),null);
  }
  for(const control of ['sketch-span','sketch-tool','sketch-select','sketch-arc-direction','sketch-arc-sector']) {
    await configure();await pointer(2,-3);await pointer(12,-3);
    await evaluate(`(()=>{const c=document.getElementById(${JSON.stringify(control)});${control==='sketch-arc-sector'?'c.checked=!c.checked':`c.value=${control==='sketch-span'?'"140"':control==='sketch-tool'?'"select"':control==='sketch-arc-direction'?'"cw"':`Array.from(c.options).find(o=>o.value!==${JSON.stringify(ids.sketch)}).value`}`};c.dispatchEvent(new Event("change",{bubbles:true}));})()`);
    assert.equal(await evaluate('arcCenter'),null);assert.equal(await evaluate('arcStart'),null);assert.equal(await evaluate('sketchDraft'),null);
    if(control==='sketch-select')await evaluate(`document.getElementById("sketch-select").value=${JSON.stringify(ids.sketch)};document.getElementById("sketch-select").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  await configure();await pointer(2,-3);await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled && !documentMutationBusy','Reload cancels arc');
  assert.equal(await evaluate('arcCenter'),null);assert.equal(source(),bytes);
  checks.push('zero radius/end direction/full turn refuse; branch/closure/view/tool/sketch/reload changes cancel partial arcs');

  await gesture(false,false,true);await review();await apply();
  const construction=logicalSource();let sketch=construction['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.equal(sketch.entities.length,4);assert.equal(sketch.entities.filter(e=>e.type==='arc'&&e.construction).length,1);assert.equal((sketch.profiles??[]).length,0);
  await undo();assert.deepEqual(logicalSource(),before);
  await evaluate('document.getElementById("redo-btn").click()');await until('!documentMutationBusy && document.getElementById("status").textContent==="Redid document edit"','Arc Redo');
  assert.deepEqual(logicalSource(),construction);await undo();assert.deepEqual(logicalSource(),before);
  checks.push('open construction arc publishes three points/one arc as one verified edit with canonical Undo/Redo and no closed profile');

  await gesture();await review();await apply();
  sketch=logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.equal(sketch.entities.length,6);assert.equal(sketch.constraints.length,0);
  assert.deepEqual(sketch.workplane,{type:'datum_plane',feature_id:'feature:datum'});
  let arc=sketch.entities.find(e=>e.type==='arc');assert.equal(arc.start_point,'ent:arc_1_start');assert.equal(arc.end_point,'ent:arc_1_end');
  assert.equal(sketch.entities.find(e=>e.id==='ent:arc_1_end').y,'7.000000000000 mm');
  await body('feature:arc_minor','471.239');
  assert.deepEqual(logicalSource()['sketches.json'].sketches.filter(s=>s.id!==ids.sketch),original['sketches.json'].sketches);
  await undo();await undo();assert.deepEqual(logicalSource(),before);
  checks.push('Close with radii shares endpoint IDs atomically; CCW native quarter-sector has 150*pi mm3 and preserves datum/prior source');

  await gesture(true);const majorDraft=await evaluate('document.querySelector("#sketch-draft-arc path").getAttribute("d").split(" ")');
  assert.equal(majorDraft[7],'1');assert.equal(majorDraft[8],'1');await review();await apply();
  sketch=logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch);arc=sketch.entities.find(e=>e.type==='arc');
  assert.equal(arc.start_point,'ent:arc_1_end');assert.equal(arc.end_point,'ent:arc_1_start');
  await body('feature:arc_major','1413.717');const major=logicalSource();
  await gesture(false,false,true);assert.equal(await evaluate('sketchDraft[0].id'),'ent:arc_2');await review();await apply();
  assert.equal(logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities.length,10);await undo();assert.deepEqual(logicalSource(),major);
  checks.push('CW selects the genuine 270 degree native sector (450*pi mm3), not the short arc; fresh IDs and one-edit history preserve existing source');

  await gesture(false,false,true);await review();const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Arc datum motion');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('sketchDraft'),null);
  const stable=source(),saved=await evaluate('JSON.stringify(documentHistoryState)'),published=await evaluate('document.getElementById("preview").src');
  await evaluate(`globalThis.__pointerArcStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__pointerArcStale="accepted";}catch{globalThis.__pointerArcStale="refused";}})()`);
  await until('globalThis.__pointerArcStale!==null','Actual stale arc refusal');assert.equal(await evaluate('globalThis.__pointerArcStale'),'refused');
  assert.equal(source(),stable);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),saved);assert.equal(await evaluate('document.getElementById("preview").src'),published);
  assert.equal(logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities.length,6);
  checks.push('upstream datum edit cancels review; actual stale arc command preserves complete source/history/published preview');
  return checks;
}
