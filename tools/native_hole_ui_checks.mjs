// ADR-111 actual product forms and unchanged real IPC on Linux and Windows.
import assert from 'node:assert/strict';
import {cpSync, realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkNativeHoles({evaluate,until,source,logicalSource,partPath}) {
  const checks=[], fixture=fileURLToPath(new URL('../examples/native_hole_stock.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !featureBusy && !sketchBusy && !sketchCreateBusy && !holeControls.busy()','Prior fixture idle');
  cpSync(fixture,partPath,{recursive:true});
  await evaluate('globalThis.__holeLoaded=false;loadDocument(currentPath).then(()=>{globalThis.__holeLoaded=true;})');
  await until('globalThis.__holeLoaded && featureTree?.rows.some(r=>r.id==="feature:sketch_bottom") && !document.getElementById("hole-review").disabled && document.getElementById("status").textContent.startsWith("Loaded ")','Fresh hole stock loaded');
  const $=id=>`document.getElementById(${JSON.stringify(id)})`;
  async function change(id,value,event='change') { await evaluate(`${$(id)}.value=${JSON.stringify(value)};${$(id)}.dispatchEvent(new Event(${JSON.stringify(event)},{bubbles:true}))`); }
  async function stable() { return {source:source(),history:await evaluate('JSON.stringify(documentHistoryState)'),preview:await evaluate(`${$('preview')}.src`)}; }
  const initial=logicalSource();
  const feature=()=>logicalSource()['features.json'].feature_nodes.find(n=>n.id==='feature:hole_1');
  async function choices(seat='none',bottom=false,depth='8 mm') {
    await change('hole-stock','feature:extrude_base');
    await change('hole-entry',bottom?'ref:face:plate_bottom':'ref:face:plate_entry');
    await change('hole-seat',seat);await change('hole-depth',depth,'input');
    await change('hole-name','Reviewed M6 hole','input');await change('hole-diameter','seat_diameter','input');
    await change('hole-seat-depth','seat_depth','input');await change('hole-angle','seat_angle','input');
  }
  async function review() {
    const before=source();await evaluate(`${$('hole-form')}.requestSubmit()`);
    await until('!holeControls.busy() && !document.getElementById("hole-preview").hidden','Hole review');assert.equal(source(),before);
  }
  async function apply() {await evaluate(`${$('hole-apply')}.click()`);await until('!holeControls.busy() && !documentMutationBusy && document.getElementById("status").textContent==="Created verified hole."','Hole apply');}
  async function history(operation) {await evaluate(`${$(operation+'-btn')}.click()`);await until(`!documentMutationBusy && document.getElementById("status").textContent===${JSON.stringify(operation==='undo'?'Undid document edit':'Redid document edit')}`,'Hole history');}
  function unchangedOriginal() {
    const now=logicalSource();assert.deepEqual(now['sketches.json'],initial['sketches.json']);assert.deepEqual(now['semantic_refs.json'],initial['semantic_refs.json']);
    assert.deepEqual(now['features.json'].feature_nodes.filter(n=>n.id!=='feature:hole_1'),initial['features.json'].feature_nodes);
  }
  await choices();
  assert.equal(await evaluate(`${$('hole-stock')}.options.length`),1);assert.equal(await evaluate(`${$('hole-entry')}.options.length`),2);
  assert.match(await evaluate(`${$('hole-axis')}.selectedOptions[0].textContent`),/ent:seat_circle/);
  assert.equal(await evaluate(`${$('hole-standard')}.selectedOptions[0].textContent`),'ISO 273 M6 normal');
  await review();assert.match(await evaluate(`${$('hole-effect')}.textContent`),/Entry ownership verified/);assert.match(await evaluate(`${$('hole-effect')}.textContent`),/bore 6\.600000 mm × 8\.000000 mm inward from top.*Plain bore.*12\.000000, 9\.000000, 10\.000000/);
  const volume=12000-Math.PI*3.3**2*8;assert.match(await evaluate(`${$('hole-effect')}.textContent`),new RegExp(volume.toFixed(6).replace('.','\\.')));
  await choices('none',true);await review();assert.match(await evaluate(`${$('hole-effect')}.textContent`),/inward from bottom.*12\.000000, 9\.000000, 0\.000000.*0\.000000, 0\.000000, 1\.000000/);
  await evaluate(`${$('hole-cancel')}.click()`);assert.deepEqual(logicalSource(),initial);
  checks.push('native stock/owned end-entry/circle-axis and matching standard dropdowns show backend SI contacts and inward direction; top/bottom plain-bore reviews are read-only and cancel preserves source');

  await choices();await review();await apply();unchangedOriginal();assert.equal(feature().definition.type,'standard_hole');assert.equal(feature().definition.depth_expr,'8 mm');
  assert.equal(await evaluate(`${$('hole-review')}.disabled`),true);
  const plain=logicalSource();await history('undo');assert.deepEqual(logicalSource(),initial);await history('redo');assert.deepEqual(logicalSource(),plain);await history('undo');
  checks.push('native plain-hole Create appends one owned standard feature, preserves original stock/axis/references, removes modified stock from choices and supports canonical Undo/Redo');

  await choices('counterbore',false,'bore_depth');await review();
  assert.match(await evaluate(`${$('hole-effect')}.textContent`),/Counterbore: diameter 11\.000000 mm, depth 4\.000000 mm/);
  assert.match(await evaluate(`${$('hole-effect')}.textContent`),/11414\.595625 mm³/);
  await apply();unchangedOriginal();assert.equal(feature().definition.seat.type,'counterbore');assert.equal(feature().definition.seat.depth_expr,'seat_depth');
  const counterbore=logicalSource();await history('undo');assert.deepEqual(logicalSource(),initial);await history('redo');assert.deepEqual(logicalSource(),counterbore);await history('undo');
  checks.push('native explicit counterbore diameter/depth review independently matches 11414.595625 mm3 remaining volume; Create preserves parameter expressions and canonical source/history');

  await choices('countersink',true,'bore_depth');await review();
  assert.match(await evaluate(`${$('hole-effect')}.textContent`),/inward from bottom.*Countersink: diameter 11\.000000 mm, depth 2\.200000 mm, included angle seat_angle/);
  await apply();unchangedOriginal();assert.equal(feature().definition.face_ref,'ref:face:plate_bottom');assert.equal(feature().definition.seat.type,'countersink');assert.equal(feature().definition.seat.included_angle_expr,'seat_angle');
  const countersink=logicalSource();await history('undo');assert.deepEqual(logicalSource(),initial);await history('redo');assert.deepEqual(logicalSource(),countersink);await history('undo');
  checks.push('native bottom-entry countersink controls review the 90-degree frustum depth 2.2 mm, apply the exact owned axis/angle source and retain canonical Undo/Redo');

  for(const [seat,id,value] of [['none','hole-depth','8'],['none','hole-depth','8 deg'],['none','hole-depth','12 mm'],['none','hole-depth','seat_angle'],['counterbore','hole-seat-depth','10 mm'],['counterbore','hole-diameter','30 mm'],['countersink','hole-angle','0 deg'],['countersink','hole-angle','180 deg']]) {
    await choices(seat,false,'bore_depth');await change(id,value,'input');const before=await stable();
    await evaluate(`${$('hole-form')}.requestSubmit()`);await until('!holeControls.busy() && document.getElementById("status").textContent.startsWith("Hole refused:")','Native hole refusal');
    assert.deepEqual(await stable(),before);assert(await evaluate(`${$('hole-preview')}.hidden`));
  }
  checks.push('native unitless/wrong-kind length, out-of-stock bore, oversized/full-depth seat and zero/flat-angle refusals preserve file bytes, history and preview publication');

  const beforeCancel=await stable();
  for(const [id,value,event] of [['hole-name','Changed name','input'],['hole-depth','7 mm','input'],['hole-diameter','12 mm','input'],['hole-seat-depth','5 mm','input'],['hole-angle','60 deg','input'],['hole-seat','none','change'],['hole-stock','feature:extrude_base','change'],['hole-entry','ref:face:plate_bottom','change'],['hole-axis','["feature:sketch_seat","sketch:seat/profile:outer"]','change'],['hole-standard','{"type":"iso273","size":"M6","fit":"normal"}','change'],['sketch-tool','circle','change'],['sketch-span','300','change']]) {
    await choices('counterbore',false,'bore_depth');await review();await change(id,value,event);assert(await evaluate(`${$('hole-preview')}.hidden`));
  }
  await choices();await evaluate(`${$('hole-form')}.requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))`);
  await until('!holeControls.busy()','Late real hole review after Escape');assert(await evaluate(`${$('hole-preview')}.hidden`));
  await choices();await evaluate(`${$('hole-form')}.requestSubmit();document.getElementById("hole-depth").value="7 mm";document.getElementById("hole-depth").dispatchEvent(new Event("input",{bubbles:true}))`);
  await until('!holeControls.busy()','Changed form rejects late real IPC');assert(await evaluate(`${$('hole-preview')}.hidden`));assert.deepEqual(await stable(),beforeCancel);
  await choices('counterbore',false,'bore_depth');await review();
  await evaluate('globalThis.__holeStale=null;(async()=>{const state=await invoke("inspect_hole_creation_cmd",{path:currentPath});const c=state.choices.find(c=>c.face_ref==="ref:face:plate_entry");globalThis.__holeStale={document_id:state.document_id,revision:state.revision,target_feature:c.target_feature,face_ref:c.face_ref,sketch_feature:c.sketch_feature,profile_ref:c.profile_ref,standard:c.standards[0],name:"Stale hole",depth:"bore_depth",seat:{type:"counterbore",diameter:"seat_diameter",depth:"seat_depth"}};})()');
  await until('globalThis.__holeStale!==null','Current actual hole intent');
  await evaluate('document.getElementById("param-param:stock_height").focus();document.getElementById("param-param:stock_height").value="12 mm";document.getElementById("param-param:stock_height").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated stock_height"','Upstream stock height changes');assert(await evaluate(`${$('hole-preview')}.hidden`));
  assert.match(await evaluate(`${$('hole-info')}.textContent`),/stock thickness 12\.000000 mm/);
  const moved=await stable();await evaluate('globalThis.__holeReplay=null;(async()=>{try{await invoke("apply_hole_create_cmd",{path:currentPath,create:globalThis.__holeStale,history:documentHistoryState?.history??null});globalThis.__holeReplay="accepted";}catch{globalThis.__holeReplay="refused";}})()');
  await until('globalThis.__holeReplay!==null','Actual stale hole apply');assert.equal(await evaluate('globalThis.__holeReplay'),'refused');assert.deepEqual(await stable(),moved);
  await choices('counterbore',false,'bore_depth');await review();assert.match(await evaluate(`${$('hole-effect')}.textContent`),/12\.000000, 9\.000000, 12\.000000/);
  await evaluate('globalThis.__holeReload=false;loadDocument(currentPath).then(()=>{globalThis.__holeReload=true;})');
  await until('globalThis.__holeReload && !holeControls.busy() && !documentMutationBusy','Completed hole reload');assert(await evaluate(`${$('hole-preview')}.hidden`));
  checks.push('native every hole field/source/tool/view change, Cancel/Escape and completed reload invalidate review; unchanged late real IPC cannot revive it, live stock/datum motion updates entry and stale actual apply preserves publication/history');
  return checks;
}
