// ADR-110 actual product DOM and unchanged real IPC, shared by both native hosts.
import assert from 'node:assert/strict';
import {cpSync,realpathSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

export async function checkSelectedOffsets({evaluate,until,source,logicalSource,partPath}) {
  const checks=[],fixture=fileURLToPath(new URL('../examples/selected_offset.ocad.d',import.meta.url));
  assert.notEqual(realpathSync(partPath),realpathSync(fixture));
  await until('!documentMutationBusy && !sketchBusy && !sketchCreateBusy && !featureBusy','Prior fixture idle');
  cpSync(fixture,partPath,{recursive:true});await evaluate('loadDocument(currentPath)');
  await until('sketchCreationState && !documentMutationBusy && document.getElementById("status").textContent.startsWith("Loaded ") && featureTree?.revision===sketchEditorState?.revision && sketchEditorState?.sketches.some(s=>s.source.id==="sketch:new_1" && s.source.entities.some(e=>e.id==="ent:offset_disc") && s.source.constraints.some(c=>c.id==="con:offset_horizontal"))','Fresh offset source loaded');
  const $=id=>`document.getElementById(${JSON.stringify(id)})`;
  async function change(id,value,event='change') {await evaluate(`${$(id)}.value=${JSON.stringify(value)};${$(id)}.dispatchEvent(new Event(${JSON.stringify(event)},{bubbles:true}))`);}
  await change('sketch-select','sketch:new_1');const initial=logicalSource();
  const original=()=>logicalSource()['sketches.json'].sketches.find(s=>s.id==='sketch:new_1');
  const copies=()=>logicalSource()['sketches.json'].sketches.filter(s=>s.id.startsWith('sketch:offset_'));
  async function stable(){return {source:source(),history:await evaluate('JSON.stringify(documentHistoryState)'),preview:await evaluate('document.getElementById("preview").src')};}
  async function choices(ids=['ent:offset_disc'],side='outward',distance='3 mm') {
    await change('sketch-tool','offset');
    for(const id of ids){await change('offset-source',id);await evaluate(`${$('offset-add')}.click()`);}
    await change('offset-side',side);await change('offset-distance',distance,'input');await change('offset-name','Independent circle offset','input');
  }
  async function review(){const before=source();await evaluate(`${$('sketch-offset-form')}.requestSubmit()`);await until('!sketchBusy && !document.getElementById("offset-preview").hidden','Offset review');assert.equal(source(),before);}
  async function apply(){await evaluate(`${$('offset-apply')}.click()`);await until('!sketchBusy && !documentMutationBusy && document.getElementById("status").textContent==="Created verified independent offset."','Offset apply');}
  async function history(operation){await evaluate(`${$(operation+'-btn')}.click()`);await until(`!documentMutationBusy && document.getElementById("status").textContent===${JSON.stringify(operation==='undo'?'Undid document edit':'Redid document edit')}`,'Offset history');}
  async function body(owner,volume) {
    const choice=await evaluate(`Array.from(${$('feature-profile')}.options).find(o=>JSON.parse(o.value).sketch_feature===${JSON.stringify(owner)})?.value`);assert(choice);
    await change('feature-profile',choice);await change('feature-id','feature:offset_body','input');await change('feature-name','Independent offset body','input');await change('feature-length','6 mm','input');
    await evaluate(`${$('feature-create')}.requestSubmit()`);await until('!featureBusy && !document.getElementById("feature-review").hidden','Offset extrusion review');
    assert.match(await evaluate(`${$('feature-effect')}.textContent`),new RegExp(volume.toFixed(3).replace('.','\\.')+' mm³'));
    await evaluate(`${$('feature-apply')}.click()`);await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent==="Applied verified feature edit."','Offset extrusion apply');
  }
  assert.deepEqual(await evaluate('sketchRow().offset_targets'),['ent:offset_disc','ent:offset_bottom','ent:offset_right','ent:offset_top','ent:offset_left']);
  await change('sketch-tool','offset');
  await evaluate('document.querySelector(\'#sketch-canvas [data-source-id="ent:offset_disc"]\').dispatchEvent(new KeyboardEvent("keydown",{key:"Enter",bubbles:true}))');
  assert.deepEqual(await evaluate('Array.from(document.querySelectorAll("#offset-curves li"),e=>e.dataset.offsetId)'),['ent:offset_disc']);
  await review();assert.match(await evaluate(`${$('offset-effect')}.textContent`),/radius 13\.000000 mm/);
  assert.match(await evaluate(`${$('offset-effect')}.textContent`),/sketch:offset_1.*feature:offset_1.*ent:offset_1_curve_1.*ent:offset_1_point_1/);
  assert.match(await evaluate(`${$('offset-effect')}.textContent`),/1884\.956 mm³/);
  assert(!await evaluate('Array.from(document.getElementById("offset-source").options).some(o=>o.value==="ent:offset_1_curve_1")'));
  await change('offset-side','inward');assert(await evaluate(`${$('offset-preview')}.hidden`));await review();assert.match(await evaluate(`${$('offset-effect')}.textContent`),/radius 7\.000000 mm/);
  assert.deepEqual(logicalSource(),initial);
  checks.push('native keyboard/canvas catalogue selection and explicit inward/outward review show backend literal dimensions and generated sketch/owner/curve/point IDs without source writes or candidate-source leakage');

  await choices();await review();await apply();const copied=logicalSource();
  assert.deepEqual(original(),initial['sketches.json'].sketches.find(s=>s.id==='sketch:new_1'));
  assert.equal(copies().length,1);assert.deepEqual(copies()[0].constraints,[]);assert.equal(copies()[0].workplane.feature_id,'feature:datum');
  assert(Math.abs(copies()[0].entities.find(e=>e.type==='circle').radius-.013)<1e-12);
  await history('undo');assert.deepEqual(logicalSource(),initial);await history('redo');assert.deepEqual(logicalSource(),copied);
  await body('feature:offset_1',1014*Math.PI);const literal=structuredClone(copies()[0]);
  await evaluate('document.getElementById("param-param:diameter").focus();document.getElementById("param-param:diameter").value="24 mm";document.getElementById("param-param:diameter").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated diameter"','Independent source parameter edit');assert.deepEqual(copies()[0],literal);
  await history('undo');await history('undo');await history('undo');assert.deepEqual(logicalSource(),initial);
  checks.push('native reviewed apply creates one independent source/owner atomically; canonical Undo/Redo, 1014*pi mm3 extrusion and source parameter edits preserve literal copy and original constraints');

  const rectangle=['ent:offset_bottom','ent:offset_right','ent:offset_top','ent:offset_left'];
  await choices([rectangle[1],rectangle[0],rectangle[2],rectangle[3]]);const unchanged=await stable();
  await evaluate(`${$('sketch-offset-form')}.requestSubmit()`);await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Offset refused:")','Unordered contour refusal');assert.deepEqual(await stable(),unchanged);
  await evaluate('document.querySelectorAll("#offset-curves li")[1].querySelector("button").click()');
  assert.deepEqual(await evaluate('Array.from(document.querySelectorAll("#offset-curves li"),e=>e.dataset.offsetId)'),rectangle);
  await review();assert.match(await evaluate(`${$('offset-effect')}.textContent`),/37\.000000, -33\.000000.*123\.000000, 33\.000000/);
  await change('offset-side','inward');await review();assert.match(await evaluate(`${$('offset-effect')}.textContent`),/43\.000000, -27\.000000.*117\.000000, 27\.000000/);
  await apply();await body('feature:offset_1',74*54*6);await history('undo');await history('undo');assert.deepEqual(logicalSource(),initial);
  await choices(rectangle);await evaluate('document.querySelector("#offset-curves li button:last-child").click()');assert.equal(await evaluate('document.querySelectorAll("#offset-curves li").length'),3);
  checks.push('native ordered curve list supports explicit move/remove and refuses broken order; outward 86x66 and inward 74x54 mm rectangle reviews and real 23976 mm3 extrusion preserve source/history');

  for(const [side,distance] of [['outward','3'],['outward','-3 mm'],['outward','0 mm'],['outward','3 deg'],['outward','diameter'],['inward','10 mm']]) {
    await choices(['ent:offset_disc'],side,distance);const before=await stable();
    await evaluate(`${$('sketch-offset-form')}.requestSubmit()`);await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Offset refused:")','Native offset refusal');
    assert.deepEqual(await stable(),before);assert(await evaluate(`${$('offset-preview')}.hidden`));
  }
  await choices(['ent:offset_bottom']);const open=await stable();await evaluate(`${$('sketch-offset-form')}.requestSubmit()`);
  await until('!sketchBusy && document.getElementById("status").textContent.startsWith("Offset refused:")','Open contour refusal');assert.deepEqual(await stable(),open);
  checks.push('native unitless/negative/zero/angle/parameter magnitude, collapsing inset and open contour refuse without source/history or preview publication');

  const stableCancel=await stable();
  for(const [id,value,event] of [['offset-distance','4 mm','input'],['offset-name','Different copy','input'],['offset-side','inward','change'],['offset-source','ent:offset_bottom','change'],['sketch-tool','select','change'],['sketch-span','400','change'],['sketch-select','sketch:plate','change']]) {
    await change('sketch-select','sketch:new_1');await choices();await review();await change(id,value,event);assert(await evaluate(`${$('offset-preview')}.hidden`));
  }
  await change('sketch-select','sketch:new_1');await choices();await review();await evaluate(`${$('offset-cancel')}.click()`);assert(await evaluate(`${$('offset-preview')}.hidden`));
  await choices();await evaluate(`${$('sketch-offset-form')}.requestSubmit();document.dispatchEvent(new KeyboardEvent("keydown",{key:"Escape",bubbles:true}))`);
  await until('!sketchBusy','Late actual offset review');assert(await evaluate(`${$('offset-preview')}.hidden`));
  await choices();await evaluate(`${$('sketch-offset-form')}.requestSubmit();document.getElementById("offset-distance").value="4 mm";document.getElementById("offset-distance").dispatchEvent(new Event("input",{bubbles:true}))`);
  await until('!sketchBusy','Changed input discards actual late review');assert(await evaluate(`${$('offset-preview')}.hidden`));assert.deepEqual(await stable(),stableCancel);
  checks.push('native source/order/distance/side/name/tool/view changes and Discard/Escape invalidate reviews; late unchanged real IPC cannot restore an applyable stale candidate');

  await choices();await review();await apply();const originalCopy=structuredClone(copies()[0]);
  await choices();await review();const stale=await evaluate('({document_id:sketchEditorState.document_id,revision:sketchEditorState.revision,source_sketch_id:"sketch:new_1",curve_ids:["ent:offset_disc"],distance:"3 mm",side:"outward",name:"Independent circle offset"})');
  await evaluate('document.getElementById("param-param:offset").focus();document.getElementById("param-param:offset").value="15 mm";document.getElementById("param-param:offset").blur()');
  await until('!documentMutationBusy && document.getElementById("status").textContent==="Updated offset"','Shared datum moves');assert(await evaluate(`${$('offset-preview')}.hidden`));assert.deepEqual(copies()[0],originalCopy);
  const row=await evaluate('sketchEditorState.sketches.find(s=>s.source.id==="sketch:offset_1")');assert(Math.abs(row.resolved_frame.origin_m[2]-.015)<1e-12);
  const moved=await stable();await evaluate(`globalThis.__offsetStale=null;(async()=>{try{await invoke("apply_sketch_offset_cmd",{path:currentPath,offset:${JSON.stringify(stale)},history:documentHistoryState?.history??null});globalThis.__offsetStale="accepted";}catch{globalThis.__offsetStale="refused";}})()`);
  await until('globalThis.__offsetStale!==null','Actual stale offset replay');assert.equal(await evaluate('globalThis.__offsetStale'),'refused');assert.deepEqual(await stable(),moved);
  await choices();await review();await evaluate('globalThis.__offsetReload=false;loadDocument(currentPath).then(()=>{globalThis.__offsetReload=true;})');
  await until('globalThis.__offsetReload && !sketchBusy && !documentMutationBusy','Completed offset reload');assert(await evaluate(`${$('offset-preview')}.hidden`));assert.equal(await evaluate('document.querySelectorAll("#offset-curves li").length'),0);
  checks.push('native upstream datum changes and completed reload clear offset drafts; symbolic workplane moves the independent literal copy, while stale actual apply preserves source/history/publication');
  return checks;
}
