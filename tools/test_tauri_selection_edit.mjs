// Actual Linux Tauri WebView interaction evidence; no browser/IPC harness.
// Node >=22, a disposable copy of ellipse_plate with the declared ellipse_top ref, and WebKit Inspector HTTP server.
// Usage: node tools/test_tauri_feature_editor.mjs http://127.0.0.1:9223 /tmp/editor/examples/bracket.ocad.d
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
const results = [];
try {
  await until('previewSelectionFrame?.bindings?.some(b => b.target.kind === "face_reference" && b.target.ref_id === "ref:face:ellipse_top")', 'Native declared face binding');
  const before = source();
  await evaluate('window.originalPickHandler = handlePickSummary; handlePickSummary = (summary, label) => { window.savedSelectionResponse = structuredClone(summary); return window.originalPickHandler(summary, label); };');
  // Dispatch a pointer event on the actual displayed PNG. The existing Tauri
  // pick command computes the hit; no inferred feature ID supplies a mutation.
  await evaluate(`(() => { const image = document.getElementById("preview"); const r = image.getBoundingClientRect(); image.dispatchEvent(new MouseEvent("click", {bubbles:true,clientX:r.left+r.width/2,clientY:r.top+r.height/2})); })()`);
  await until('selectedBinding?.target?.ref_id === "ref:face:ellipse_top" && !document.getElementById("selection-edit").hidden', 'Actual preview pick');
  assert.equal(await evaluate('selectedBinding.feature_id'), 'feature:ellipse_body');
  await evaluate('window.savedSelectionResponse.selection.inferred_feature_id="feature:WRONG_NAME_HINT"; window.savedSelectionResponse.selection.inferred_topo_ref_id="ref:WRONG_HINT"; handlePickSummary(window.savedSelectionResponse,"preview");');
  await review('document.getElementById("selection-expr").value="10 mm"; document.getElementById("selection-edit").requestSubmit();');
  assert.equal(source(), before);
  assert.match(await evaluate('document.getElementById("feature-effect").textContent'), /18849\.556 mm³/);
  await apply();
  const after = logicalSource();
  const body = after['features.json'].feature_nodes.find(n=>n.id==='feature:ellipse_body');
  assert.equal(body.definition.length_expr, '10 mm');
  assert.equal(body.definition.sketch_feature, 'feature:ellipse_sketch');
  assert.equal(body.definition.profile_ref, 'sketch:ellipse/profile:outer');
  results.push('actual preview face pick → declared semantic ref → verified dimension form');
  const stable = source();
  const history = await evaluate('JSON.stringify(documentHistoryState)');
  await evaluate('handlePickSummary(window.savedSelectionResponse, "preview")');
  assert.equal(await evaluate('document.getElementById("selection-edit").hidden'), true);
  assert.equal(source(), stable);
  assert.equal(await evaluate('JSON.stringify(documentHistoryState)'), history);
  results.push('delayed old pick response and wrong name hints cannot authorize editing');
  await evaluate('document.getElementById("undo-btn").click()');
  await until('document.getElementById("status").textContent === "Undid document edit"', 'Undo');
  assert.equal(logicalSource()['features.json'].feature_nodes.find(n=>n.id==='feature:ellipse_body').definition.length_expr, 'thickness');
  await evaluate('document.getElementById("redo-btn").click()');
  await until('document.getElementById("status").textContent === "Redid document edit"', 'Redo');
  assert.deepEqual(logicalSource(), after);
  results.push('opaque backend undo/redo preserves source identities');
  console.log(JSON.stringify({actual_tauri_semantic_selection_interactions:'passed',results},null,2));
} finally { socket.close(); }
