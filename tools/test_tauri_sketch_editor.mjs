// Actual Linux Tauri WebView interaction evidence; no browser/IPC harness.
// Node >=22, a disposable copy of ellipse_plate with the declared ellipse_top ref, and WebKit Inspector HTTP server.
// Usage: node tools/test_tauri_sketch_editor.mjs http://127.0.0.1:9223 /tmp/editor/examples/bracket.ocad.d
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const [endpoint, partPath] = process.argv.slice(2);
assert(endpoint && partPath, 'Inspector HTTP endpoint and disposable part path required');
const page = await (await fetch(endpoint)).text();
const path = page.match(/\/socket\/\d+\/\d+\/WebPage/)?.[0];
assert(path, 'No actual Tauri inspectable page');
const socket = new WebSocket(endpoint.replace(/^http/, 'ws') + path);
let target;
let sequence = 0;
const pending = new Map();
const ready = new Promise(resolve => {
  socket.addEventListener('message', ({data}) => {
    const event = JSON.parse(data);
    if (event.method === 'Target.targetCreated') { target = event.params.targetInfo.targetId; resolve(); }
    if (event.method === 'Target.dispatchMessageFromTarget') {
      const result = JSON.parse(event.params.message);
      const waiter = pending.get(result.id);
      if (waiter) { pending.delete(result.id); waiter(result); }
    }
  });
});
await Promise.race([ready, new Promise((_, reject) => setTimeout(() => reject(new Error('Inspector timeout')), 10000))]);
async function evaluate(expression) {
  const id = ++sequence;
  const response = new Promise(resolve => pending.set(id, resolve));
  socket.send(JSON.stringify({id: id + 10000, method:'Target.sendMessageToTarget', params:{targetId:target, message:JSON.stringify({id, method:'Runtime.evaluate',params:{expression,returnByValue:true}})}}));
  const result = await Promise.race([response, new Promise((_, reject) => setTimeout(() => reject(new Error('Evaluation timeout')), 10000))]);
  assert(!result.error && !result.result?.wasThrown, JSON.stringify(result));
  return result.result.result.value;
}
async function until(expression, description) {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if (await evaluate(expression)) return;
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  throw new Error(`${description}: ${await evaluate('document.getElementById("status").textContent')}`);
}
function logicalSource() {
  return Object.fromEntries(readdirSync(join(partPath, "graph")).sort().map(file => [file, JSON.parse(readFileSync(join(partPath, "graph", file), "utf8"))]));
}
function source() {
  return JSON.stringify(readdirSync(join(partPath, 'graph')).sort().map(file => [file, readFileSync(join(partPath,'graph',file),'utf8')]).concat([['checksums.json',readFileSync(join(partPath,'checksums.json'),'utf8')]]));
}
async function review(expression) {
  const before = source();
  await evaluate(expression);
  await until('!document.getElementById("feature-review").hidden && !featureBusy', 'Review');
  assert.equal(source(), before, 'Review must not write source');
}
async function apply() {
  await evaluate('document.getElementById("feature-apply").click()');
  await until('!featureBusy && document.getElementById("status").textContent === "Applied verified feature edit."', 'Apply');
}

try {
  await until('sketchEditorState?.sketches.length === 1 && !documentMutationBusy', 'Sketch loaded');
  const original=logicalSource();
  // Select the actual analytic ellipse in the SVG (its source ID is backend-authored).
  await evaluate('document.querySelector("#sketch-canvas ellipse").dispatchEvent(new MouseEvent("click",{bubbles:true}));');
  assert.equal(await evaluate('document.getElementById("sketch-id").value'),'ent:ellipse');
  const before=source();
  await evaluate(`document.getElementById("sketch-x").value="40 mm";document.getElementById("sketch-y").value="15 mm";document.getElementById("sketch-angle").value="0 deg";document.getElementById("sketch-form").requestSubmit();`);
  await until('!document.getElementById("sketch-review").hidden && !sketchBusy','Native sketch review');
  assert.equal(source(),before);assert.match(await evaluate('document.getElementById("sketch-effect").textContent'),/11309.734 mm³/);
  await evaluate('document.getElementById("sketch-apply").click()');await until('!sketchBusy && document.getElementById("status").textContent==="Applied verified sketch edit."','Apply sketch');
  const changed=logicalSource();assert.equal(changed['sketches.json'].sketches[0].entities.find(e=>e.id==='ent:ellipse').major_radius,'40 mm');
  // Select a source point, then click the canvas to populate explicit local-mm coordinates.
  await evaluate(`document.querySelector('#sketch-canvas [data-source-id="ent:center"]').dispatchEvent(new MouseEvent("click",{bubbles:true}));
    (()=>{const c=document.getElementById("sketch-canvas"),r=c.getBoundingClientRect();c.dispatchEvent(new MouseEvent("click",{bubbles:true,clientX:r.left+r.width*.57,clientY:r.top+r.height*.44}));})();document.getElementById("sketch-form").requestSubmit();`);
  await until('!document.getElementById("sketch-review").hidden&&!sketchBusy','Point move review');
  await evaluate('document.getElementById("sketch-apply").click()');await until('!sketchBusy&&document.getElementById("status").textContent==="Applied verified sketch edit."','Point move apply');
  const moved=logicalSource()['sketches.json'].sketches[0].entities.find(e=>e.id==='ent:center');assert.equal(typeof moved.x,'string');assert.match(moved.x,/ mm$/);assert.notEqual(moved.x,'0 mm');
  await evaluate('document.getElementById("undo-btn").click()');await until('!documentMutationBusy','Point move undo');assert.deepEqual(logicalSource(),changed);
  // Two pointer clicks draft a construction line, with a visible uncommitted overlay.
  const beforeLine=source();await evaluate(`document.getElementById("sketch-tool").value="line";document.getElementById("sketch-construction").checked=true;
    (()=>{const c=document.getElementById("sketch-canvas"),r=c.getBoundingClientRect();for(const [x,y] of [[.3,.2],[.65,.23]])c.dispatchEvent(new MouseEvent("click",{bubbles:true,clientX:r.left+r.width*x,clientY:r.top+r.height*y}));})();`);
  assert.equal(await evaluate('sketchDraft.length'),3);assert.equal(await evaluate('!!document.getElementById("sketch-draft-line")'),true);assert.equal(source(),beforeLine);
  await evaluate('document.getElementById("sketch-form").requestSubmit()');await until('!document.getElementById("sketch-review").hidden&&!sketchBusy','Line review');assert.equal(source(),beforeLine);
  await evaluate('document.getElementById("sketch-apply").click()');await until('!sketchBusy&&document.getElementById("status").textContent==="Applied verified sketch edit."','Line apply');
  assert.equal(logicalSource()['sketches.json'].sketches[0].entities.find(e=>e.id==='ent:line_1').construction,true);
  // Constraint form uses the stable line ID, not a renderer index.
  await evaluate(`document.getElementById("sketch-action").value="horizontal";document.getElementById("sketch-id").value="con:horizontal";document.getElementById("sketch-a").value="ent:line_1";document.getElementById("sketch-form").requestSubmit();`);
  await until('!document.getElementById("sketch-review").hidden&&!sketchBusy','Constraint review');await evaluate('document.getElementById("sketch-apply").click()');await until('!sketchBusy&&document.getElementById("status").textContent==="Applied verified sketch edit."','Constraint apply');
  assert.equal(logicalSource()['sketches.json'].sketches[0].constraints[0].type,'horizontal');
  const solved=await evaluate('sketchRow().solved.entities.filter(e=>e.id==="ent:line_1_a"||e.id==="ent:line_1_b").map(e=>e.y)');assert(Math.abs(solved[0]-solved[1])<1e-8);
  const publishedPreview=await evaluate('document.getElementById("preview").src');
  const committed=logicalSource(),stable=source(),history=await evaluate('JSON.stringify(documentHistoryState)');
  await evaluate(`document.getElementById("sketch-action").value="point";document.getElementById("sketch-id").value="ent:center";document.getElementById("sketch-x").value="10";document.getElementById("sketch-form").requestSubmit();`);
  await until('!sketchBusy&&document.getElementById("status").textContent.startsWith("Sketch refused:")','Bad unit refusal');assert.equal(source(),stable);assert.equal(await evaluate('document.getElementById("preview").src'),publishedPreview);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);
  for(let i=0;i<3;i++){await evaluate('document.getElementById("undo-btn").click()');await until('!documentMutationBusy','Undo');}assert.deepEqual(logicalSource(),original);
  for(let i=0;i<3;i++){await evaluate('document.getElementById("redo-btn").click()');await until('!documentMutationBusy','Redo');}assert.deepEqual(logicalSource(),committed);
  console.log(JSON.stringify({passed:true,actualTauri:true,checks:['analytic ellipse pointer/form edit','source-point selection and move','two-click draft and atomic line commit','constraint solve','explicit-unit refusal preserves source/history','undo/redo source identity']}));
} finally {socket.close();}
