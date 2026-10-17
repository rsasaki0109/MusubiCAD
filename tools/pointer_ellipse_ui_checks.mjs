// Actual native WebKit/WebView2 events, product handlers and IPC (ADR-105).
import assert from 'node:assert/strict';
import {cpSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkPointerEllipses({evaluate,until,source,logicalSource,partPath}) {
  const checks=[],fixture=fileURLToPath(new URL('../examples/datum_plate.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Previous fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState?.suggested_sketch_id==="sketch:new_1" && !document.getElementById("sketch-new-review").disabled','Ellipse fixture loaded');
  const original=logicalSource(),ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
  await evaluate('document.getElementById("sketch-new-name").value="Pointer ellipses";document.getElementById("sketch-new-plane").value="datum:feature:datum";document.getElementById("sketch-new-form").requestSubmit()');
  await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden','Ellipse sketch review');
  await evaluate('document.getElementById("sketch-new-apply").click()');
  await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified sketch."','Ellipse sketch creation');
  const before=logicalSource(),bytes=source(),history=await evaluate('JSON.stringify(documentHistoryState)'),preview=await evaluate('document.getElementById("preview").src');
  async function pointer(x,y,type='click') {
    // Align presentation only for integer WebKit MouseEvent CSS coordinates.
    // No input property, product math, typed intent or IPC result is replaced.
    await evaluate(`(()=>{const c=document.getElementById("sketch-canvas");c.style.transform="none";const m=c.getScreenCTM();c.style.transform=\`translate(\${Math.round(m.e)-m.e}px,\${Math.round(m.f)-m.f}px)\`;const p=c.createSVGPoint();p.x=${x};p.y=${-y};const v=p.matrixTransform(c.getScreenCTM());c.dispatchEvent(new ${type==='pointermove'?'PointerEvent':'MouseEvent'}(${JSON.stringify(type)},{bubbles:true,clientX:v.x,clientY:v.y}));})()`);
  }
  async function configure(construction=false) {
    await evaluate(`document.getElementById("sketch-span").value="120";document.getElementById("sketch-tool").value="ellipse";document.getElementById("sketch-construction").checked=${construction};document.getElementById("sketch-tool").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  async function gesture(construction=false) {
    await configure(construction);await pointer(2,-3);await pointer(26,15);await pointer(14,21,'pointermove');await pointer(14,21);
  }
  async function review() {
    const stable=source();await evaluate('document.getElementById("sketch-form").requestSubmit()');
    await until('!sketchBusy && !document.getElementById("sketch-review").hidden','Ellipse review');assert.equal(source(),stable);
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-apply").click()');
    await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Ellipse apply');
  }
  async function undo() {
    await evaluate('document.getElementById("undo-btn").click()');
    await until('!documentMutationBusy && document.getElementById("status").textContent==="Undid document edit"','Ellipse Undo');
  }
  async function body(id) {
    await evaluate(`document.getElementById("feature-id").value=${JSON.stringify(id)};document.getElementById("feature-name").value="Pointer ellipse prism";document.getElementById("feature-length").value="6 mm";document.getElementById("feature-profile").value=Array.from(document.getElementById("feature-profile").options).find(o=>o.value && JSON.parse(o.value).sketch_feature===${JSON.stringify(ids.feature)}).value;document.getElementById("feature-create").requestSubmit()`);
    await until('!featureBusy && !document.getElementById("feature-review").hidden','Ellipse solid review');
    assert((await evaluate('document.getElementById("feature-effect").textContent')).includes('11309.734 mm³'));
    await evaluate('document.getElementById("feature-apply").click()');
    await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Ellipse solid apply');
  }
  await configure();await pointer(2,-3);
  await evaluate('document.getElementById("sketch-form").requestSubmit()');
  assert.match(await evaluate('document.getElementById("status").textContent'),/^Finish ellipse/);
  await pointer(26,15);await pointer(14,21,'pointermove');
  const shape=await evaluate('(()=>{const e=document.querySelector("#sketch-draft-ellipse ellipse");return {cx:e.getAttribute("cx"),cy:e.getAttribute("cy"),a:e.getAttribute("rx"),b:e.getAttribute("ry"),transform:e.getAttribute("transform")};})()');
  assert.equal(Number(shape.cx),2);assert.equal(Number(shape.cy),3);assert(Math.abs(Number(shape.a)-30)<1e-10 && Math.abs(Number(shape.b)-20)<1e-10);
  assert(Math.abs(Number(/rotate\(([^ ]+)/.exec(shape.transform)[1])+Math.atan2(3,4)*180/Math.PI)<1e-10);
  assert.equal(await evaluate('sketchDraft'),null);
  await evaluate('document.getElementById("sketch-form").requestSubmit()');
  assert.match(await evaluate('document.getElementById("status").textContent'),/^Finish ellipse/);
  await evaluate('document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  assert.equal(await evaluate('ellipseCenter'),null);assert.equal(await evaluate('ellipseMajor'),null);assert.equal(await evaluate('!!document.getElementById("sketch-draft-ellipse")'),false);
  await gesture();
  assert.deepEqual(await evaluate('sketchDraft'),[{operation:'ellipse_axes',id:'ent:ellipse_1',center_x:'2.000 mm',center_y:'-3.000 mm',major_x:'26.000 mm',major_y:'15.000 mm',through_x:'14.000 mm',through_y:'21.000 mm',construction:false}]);
  await evaluate('document.getElementById("sketch-form").requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))');
  await until('!sketchBusy','Canceled real ellipse review');assert(await evaluate('document.getElementById("sketch-review").hidden'));
  await gesture();await evaluate('document.getElementById("sketch-clear").click();document.getElementById("sketch-form").requestSubmit()');
  assert.match(await evaluate('document.getElementById("status").textContent'),/^Finish ellipse/);
  assert.equal(source(),bytes);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);assert.equal(await evaluate('document.getElementById("preview").src'),preview);
  checks.push('actual centre/major/point-on-ellipse events preview radii 30/20 mm and orientation; half gestures, Escape, discard and late real IPC preserve source/history/preview');

  for(const failure of ['major','axis','endpoint','span','minor']) {
    await configure();await pointer(2,-3);
    if(failure==='major')await pointer(2,-3);
    else {await pointer(26,15);const q=failure==='axis'?[18,9]:failure==='endpoint'?[26,15]:failure==='span'?[42,27]:[-22,29];await pointer(...q);}
    assert.match(await evaluate('document.getElementById("status").textContent'),/^Ellipse refused:/);assert.equal(await evaluate('sketchDraft'),null);
  }
  for(const control of ['sketch-span','sketch-tool','sketch-select']) {
    await configure();await pointer(2,-3);await pointer(26,15);
    await evaluate(`(()=>{const c=document.getElementById(${JSON.stringify(control)});c.value=${control==='sketch-span'?'"140"':control==='sketch-tool'?'"select"':`Array.from(c.options).find(o=>o.value!==${JSON.stringify(ids.sketch)}).value`};c.dispatchEvent(new Event("change",{bubbles:true}));})()`);
    assert.equal(await evaluate('ellipseCenter'),null);assert.equal(await evaluate('ellipseMajor'),null);assert.equal(await evaluate('sketchDraft'),null);
    if(control==='sketch-select')await evaluate(`document.getElementById("sketch-select").value=${JSON.stringify(ids.sketch)};document.getElementById("sketch-select").dispatchEvent(new Event("change",{bubbles:true}))`);
  }
  await configure();await pointer(2,-3);await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled && !documentMutationBusy','Reload cancels ellipse');
  assert.equal(await evaluate('ellipseCenter'),null);assert.equal(source(),bytes);
  checks.push('collapsed/ambiguous/outside-axis/oversized-minor gestures refuse; view/tool/sketch/reload changes cancel partial ellipses');

  await gesture(true);await review();await apply();
  const construction=logicalSource();let sketch=construction['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.equal(sketch.entities.length,2);assert.equal(sketch.entities.filter(e=>e.type==='ellipse'&&e.construction).length,1);assert.equal((sketch.profiles??[]).length,0);
  await undo();assert.deepEqual(logicalSource(),before);
  await evaluate('document.getElementById("redo-btn").click()');await until('!documentMutationBusy && document.getElementById("status").textContent==="Redid document edit"','Ellipse Redo');
  assert.deepEqual(logicalSource(),construction);await undo();assert.deepEqual(logicalSource(),before);
  checks.push('construction ellipse creates exactly one centre and one ellipse in one verified edit; no profile/body and canonical Undo/Redo');

  await gesture();await review();await apply();
  sketch=logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch);
  assert.equal(sketch.entities.length,2);assert.equal(sketch.constraints.length,0);assert.deepEqual(sketch.workplane,{type:'datum_plane',feature_id:'feature:datum'});
  const ellipse=sketch.entities.find(e=>e.type==='ellipse');assert.equal(ellipse.center,'ent:ellipse_1_center');assert.equal(ellipse.major_radius,'30.000000000000 mm');assert.equal(ellipse.minor_radius,'20.000000000000 mm');
  await body('feature:ellipse_body');
  assert.deepEqual(logicalSource()['sketches.json'].sketches.filter(s=>s.id!==ids.sketch),original['sketches.json'].sketches);
  await undo();await undo();assert.deepEqual(logicalSource(),before);
  checks.push('through point yields minor radius 20 mm rather than projected width 12 mm; native 3600*pi mm3 prism preserves datum and prior source');

  await gesture();await review();await apply();await body('feature:ellipse_persistent');const solid=logicalSource();
  await gesture(true);assert.equal(await evaluate('sketchDraft[0].id'),'ent:ellipse_2');await review();await apply();
  assert.equal(logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities.length,4);await undo();assert.deepEqual(logicalSource(),solid);
  await evaluate('document.getElementById("sketch-tool").value="select";document.getElementById("sketch-tool").dispatchEvent(new Event("change",{bubbles:true}));Array.from(document.querySelectorAll("#sketch-canvas ellipse")).find(e=>e.dataset.sourceId==="ent:ellipse_1").dispatchEvent(new MouseEvent("click",{bubbles:true}));document.getElementById("sketch-x").value="60 mm";document.getElementById("sketch-y").value="20 mm";document.getElementById("sketch-angle").value="15 deg";document.getElementById("sketch-x").dispatchEvent(new Event("input",{bubbles:true}))');
  await review();assert((await evaluate('document.getElementById("sketch-effect").textContent')).includes('22619.467 mm³'));await apply();await undo();assert.deepEqual(logicalSource(),solid);
  checks.push('fresh IDs preserve existing ellipse geometry; direct source selection retains legacy numeric axes/angle editing and one-edit Undo');

  await gesture(true);await review();const stale=await evaluate('reviewedSketch.edit');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Ellipse datum motion');
  assert(await evaluate('document.getElementById("sketch-review").hidden'));assert.equal(await evaluate('sketchDraft'),null);
  const stable=source(),saved=await evaluate('JSON.stringify(documentHistoryState)'),published=await evaluate('document.getElementById("preview").src');
  await evaluate(`globalThis.__pointerEllipseStale=null;(async()=>{try{await invoke("apply_sketch_edit_cmd",{path:currentPath,edit:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__pointerEllipseStale="accepted";}catch{globalThis.__pointerEllipseStale="refused";}})()`);
  await until('globalThis.__pointerEllipseStale!==null','Actual stale ellipse refusal');assert.equal(await evaluate('globalThis.__pointerEllipseStale'),'refused');
  assert.equal(source(),stable);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),saved);assert.equal(await evaluate('document.getElementById("preview").src'),published);
  assert.equal(logicalSource()['sketches.json'].sketches.find(s=>s.id===ids.sketch).entities.length,2);
  checks.push('upstream datum edit cancels ellipse review; actual stale command preserves complete source/history/published preview');
  return checks;
}
