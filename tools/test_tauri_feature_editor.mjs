// Actual Linux Tauri WebView interaction evidence; no browser/IPC harness.
// Node >=22, a disposable copy of cubic_plate, and WebKit Inspector HTTP server.
// Usage: node tools/test_tauri_feature_editor.mjs http://127.0.0.1:9223 /tmp/editor/examples/bracket.ocad.d
import assert from 'node:assert/strict';
import {checkSketchCreation} from './sketch_creation_ui_checks.mjs';
import {checkDatumSketches} from './datum_sketch_ui_checks.mjs';
import {checkPointerCircles} from './pointer_circle_ui_checks.mjs';
import {checkPointerRectangles} from './pointer_rectangle_ui_checks.mjs';
import {checkPointerArcs} from './pointer_arc_ui_checks.mjs';
import {checkPointerEllipses} from './pointer_ellipse_ui_checks.mjs';
import {checkSelectedConstraints} from './selected_constraint_ui_checks.mjs';
import {checkSelectedConstruction} from './selected_construction_ui_checks.mjs';
import {checkSelectedCurveEdits} from './selected_curve_edit_ui_checks.mjs';
import {checkSelectedOffsets} from './selected_offset_ui_checks.mjs';
import {checkNativeHoles} from './native_hole_ui_checks.mjs';
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
const row = id => `document.querySelector('[data-feature-id="${id}"]')`;
const click = (id,text) => `Array.from(${row(id)}.querySelectorAll("button")).find(b => b.textContent === ${JSON.stringify(text)}).click()`;
const results = [];
try {
  await until('typeof featureTree !== "undefined" && featureTree?.rows?.length === 2 && document.getElementById("status").textContent === "Loaded Closed cubic plate"', 'Initial part load');
  await review('document.getElementById("feature-id").value="feature:second"; document.getElementById("feature-name").value="Second cubic prism"; document.getElementById("feature-length").value="12 mm"; document.getElementById("feature-create").requestSubmit();');
  await apply(); results.push('create via product form');
  await review(`${row('feature:second')}.querySelector("input").value="10 mm"; ${click('feature:second','Review dimension')}`);
  assert.match(await evaluate('document.getElementById("feature-effect").textContent'), /2400\.000 mm³/);
  await apply(); results.push('edit length with analytic 2400 mm³ preview');
  await review(click('feature:second','Review move up'));
  await apply();
  assert.deepEqual(await evaluate('featureTree.rows.map(r=>r.id)'), ['feature:cubic_sketch','feature:second','feature:cubic_body']);
  results.push('reorder with stable source IDs');
  await review(click('feature:cubic_body','Review suppress'));
  await apply(); results.push('suppress leaf feature');
  const after = logicalSource();
  await evaluate('document.getElementById("undo-btn").click()');
  await until('document.getElementById("status").textContent === "Undid document edit"', 'Undo');
  assert.equal(await evaluate('featureTree.rows.find(r=>r.id==="feature:cubic_body").suppressed'), false);
  await evaluate('document.getElementById("redo-btn").click()');
  await until('document.getElementById("status").textContent === "Redid document edit"', 'Redo');
  assert.deepEqual(logicalSource(), after); results.push('undo/redo restores canonical source identity');
  const restored = source();
  await evaluate(click('feature:cubic_sketch','Review suppress'));
  await until('!featureBusy && document.getElementById("status").textContent.startsWith("Edit refused:")', 'Invalid suppression');
  assert.equal(source(), restored);
  await evaluate(`${row('feature:second')}.querySelector("input").value="90 deg"; ${click('feature:second','Review dimension')}`);
  await until('!featureBusy && document.getElementById("status").textContent.startsWith("Edit refused:")', 'Invalid unit');
  assert.equal(source(), restored);
  results.push('invalid dependency and unit retain source/history');
  results.push(...await checkSketchCreation({evaluate,until,source,logicalSource}));
  results.push(...await checkDatumSketches({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkPointerCircles({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkPointerRectangles({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkPointerArcs({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkPointerEllipses({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkSelectedConstraints({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkSelectedConstruction({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkSelectedCurveEdits({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkSelectedOffsets({evaluate,until,source,logicalSource,partPath}));
  results.push(...await checkNativeHoles({evaluate,until,source,logicalSource,partPath}));
  console.log(JSON.stringify({actual_tauri_editor_interactions_match_verified_backend_results:'passed',results}, null, 2));
} finally { socket.close(); }
