// Actual Windows Tauri/WebView2 forms via loopback CDP; no browser/IPC mocks.
// Node >=22; run through tools/test_windows_desktop_ui.ps1 on a disposable fixture.
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
import { readFileSync, readdirSync, realpathSync } from 'node:fs';
import { join } from 'node:path';

// Also annotate failures during inspector discovery before the UI checks start.
process.on('uncaughtExceptionMonitor', error => {
  const detail = String(error.stack ?? error).replaceAll('%','%25').replaceAll('\r','%0D').replaceAll('\n','%0A');
  console.log(`::error::Windows native UI: ${detail}`);
});

const [endpoint, partPath] = process.argv.slice(2);
assert(endpoint && partPath, 'Inspector HTTP endpoint and disposable part path required');
const address = new URL(endpoint);
assert(['127.0.0.1', 'localhost', '[::1]'].includes(address.hostname), 'Inspector must use loopback');
const targets = await (await fetch(new URL('/json/list', endpoint), {signal:AbortSignal.timeout(10000)})).json();
const pages = targets.filter(page => page.type === 'page' && /^https?:\/\/tauri\.localhost(?:\/|$)/.test(page.url));
assert.equal(pages.length, 1, 'Expected exactly one actual Tauri WebView2 page');
const debugAddress = new URL(pages[0].webSocketDebuggerUrl);
assert(['127.0.0.1', 'localhost', '[::1]'].includes(debugAddress.hostname), 'WebSocket must use loopback');
const socket = new WebSocket(debugAddress);
const pending = new Map();
let sequence = 0;
socket.addEventListener('message', ({data}) => {
  const result = JSON.parse(data);
  const waiter = pending.get(result.id);
  if (waiter) { pending.delete(result.id); waiter(result); }
});
await new Promise((resolve,reject) => {
  const timer = setTimeout(() => { socket.close(); reject(new Error('WebView2 connection timeout')); }, 10000);
  socket.addEventListener('open', () => {clearTimeout(timer); resolve();}, {once:true});
  socket.addEventListener('error', () => {clearTimeout(timer); reject(new Error('WebView2 connection failed'));}, {once:true});
});
async function evaluate(expression) {
  const id = ++sequence;
  const response = new Promise(resolve => pending.set(id, resolve));
  socket.send(JSON.stringify({id, method:'Runtime.evaluate',params:{expression,returnByValue:true}}));
  let timer;
  try {
    const result = await Promise.race([response, new Promise((_,reject) => {
      timer=setTimeout(() => reject(new Error('Evaluation timeout')),10000);
    })]);
    assert(!result.error && !result.result?.exceptionDetails, JSON.stringify(result));
    return result.result.result.value;
  } finally {clearTimeout(timer);pending.delete(id);}
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
  return JSON.stringify(readdirSync(join(partPath, 'graph')).sort().map(file => [file, readFileSync(join(partPath,'graph',file),'utf8')]).concat(['checksums.json','manifest.ocad.json','document.ocad.json'].map(file => [file,readFileSync(join(partPath,file),'utf8')])));
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
  assert.equal(await evaluate('typeof window.__TAURI__?.core?.invoke'), 'function', 'Real Tauri IPC required');
  await until('typeof currentPath === \"string\" && currentPath.length > 0', 'Disposable source path');
  // Windows may expose the same temp directory through a short-name/case alias.
  assert.equal(realpathSync.native(await evaluate('currentPath')).toLowerCase(),
    realpathSync.native(partPath).toLowerCase(), 'Actual UI must edit the disposable directory');
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
  const restoredHistory = await evaluate('JSON.stringify(documentHistoryState)');
  await evaluate(click('feature:cubic_sketch','Review suppress'));
  await until('!featureBusy && document.getElementById("status").textContent.startsWith("Edit refused:")', 'Invalid suppression');
  assert.equal(source(), restored);
  await evaluate(`${row('feature:second')}.querySelector("input").value="90 deg"; ${click('feature:second','Review dimension')}`);
  await until('!featureBusy && document.getElementById("status").textContent.startsWith("Edit refused:")', 'Invalid unit');
  assert.equal(source(), restored);
  assert.equal(await evaluate('JSON.stringify(documentHistoryState)'), restoredHistory, 'Refusal must retain opaque UI history');
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
  console.log(JSON.stringify({actual_windows_tauri_webview2_forms:'passed',webview_url:pages[0].url,results}, null, 2));
} finally { socket.close(); }
