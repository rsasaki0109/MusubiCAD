// Shared checks executed against real Linux WebKit / Windows WebView2 Tauri IPC.
import assert from 'node:assert/strict';

export async function checkSketchCreation({evaluate, until, source, logicalSource}) {
  const checks=[];
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled', 'Creation controls loaded');
  const original=logicalSource(), bytes=source(), history=await evaluate('JSON.stringify(documentHistoryState)');
  async function review(plane,name='Manufacturing sketch') {
    await evaluate(`document.getElementById("sketch-new-name").value=${JSON.stringify(name)};document.getElementById("sketch-new-plane").value=${JSON.stringify(plane)};document.getElementById("sketch-new-form").requestSubmit()`);
    await until('!sketchCreateBusy && !document.getElementById("sketch-new-preview").hidden', 'Review new sketch');
  }
  async function apply() {
    await evaluate('document.getElementById("sketch-new-apply").click()');
    await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent === "Created verified sketch."', 'Create new sketch');
  }
  await review('XZ');assert.equal(source(),bytes);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);
  await evaluate('document.getElementById("sketch-new-cancel").click()');
  assert(await evaluate('document.getElementById("sketch-new-preview").hidden'));assert.equal(source(),bytes);
  // The change happens synchronously before the real asynchronous IPC reply.
  await evaluate('document.getElementById("sketch-new-form").requestSubmit();document.getElementById("sketch-new-plane").value="YZ";document.getElementById("sketch-new-plane").dispatchEvent(new Event("change",{bubbles:true}))');
  await until('!sketchCreateBusy', 'Late creation review discarded');
  assert(await evaluate('document.getElementById("sketch-new-preview").hidden'));assert.equal(source(),bytes);
  checks.push('native creation review/cancel/change discards late real IPC without writes');
  const state=await evaluate('JSON.stringify(sketchCreationState)');
  const preview=await evaluate('document.getElementById("preview").src');
  for (const mutation of ['sketchCreationState.document_id="doc:foreign"','sketchCreationState.suggested_feature_id=featureTree.rows[0].id']) {
    await evaluate(`sketchCreationState=JSON.parse(${JSON.stringify(state)});${mutation};document.getElementById("sketch-new-form").requestSubmit()`);
    await until('!sketchCreateBusy && document.getElementById("status").textContent.startsWith("Sketch creation refused:")','Foreign identity or late owner collision refused');
    assert.equal(source(),bytes);assert.equal(await evaluate('JSON.stringify(documentHistoryState)'),history);
    assert.equal(await evaluate('document.getElementById("preview").src'),preview);
  }
  await evaluate(`sketchCreationState=JSON.parse(${JSON.stringify(state)})`);
  checks.push('actual backend identity/collision refusal preserves source/history/published preview');
  for (const plane of ['XY','XZ','YZ']) {
    const before=logicalSource(),beforeBytes=source();
    const ids=await evaluate('({sketch:sketchCreationState.suggested_sketch_id,feature:sketchCreationState.suggested_feature_id})');
    await review(plane,`New ${plane} sketch`);assert.equal(source(),beforeBytes);
    assert.match(await evaluate('document.getElementById("sketch-new-effect").textContent'),new RegExp(plane));
    await apply();
    const after=logicalSource();
    const sketches=after['sketches.json'].sketches;
    const created=sketches.find(s=>s.id===ids.sketch);
    assert.deepEqual(created.workplane,{type:'global',plane});assert.equal(created.entities.length,0);
    assert.deepEqual(sketches.slice(0,-1),before['sketches.json'].sketches);
    const nodes=after['features.json'].feature_nodes;
    const owner=nodes.find(n=>n.id===ids.feature);
    assert.deepEqual(owner.definition,{type:'sketch',sketch_id:ids.sketch});assert.equal(owner.suppressed,false);
    assert.deepEqual(nodes.slice(0,-1),before['features.json'].feature_nodes);
    assert.equal(await evaluate('document.getElementById("sketch-select").value'),ids.sketch);
    assert.deepEqual(after['parameters.json'],original['parameters.json']);
    await evaluate('document.getElementById("undo-btn").click()');
    await until('!documentMutationBusy && document.getElementById("status").textContent === "Undid document edit"','Undo sketch creation');
    assert.deepEqual(logicalSource(),before);
    await evaluate('document.getElementById("redo-btn").click()');
    await until('!documentMutationBusy && document.getElementById("status").textContent === "Redid document edit"','Redo sketch creation');
    assert.deepEqual(logicalSource(),after);
  }
  checks.push('all three global planes create one source/owner atomically, select the new sketch and preserve canonical undo/redo');
  // The earlier feature-editor fixture leaves feature:second as the active body.
  await evaluate('Array.from(document.querySelector(\'[data-feature-id="feature:second"]\').querySelectorAll("button")).find(b=>b.textContent==="Review suppress").click()');
  await until('!featureBusy && !document.getElementById("feature-review").hidden','Review sketch-only state');
  await evaluate('document.getElementById("feature-apply").click()');
  await until('!featureBusy && !documentMutationBusy && document.getElementById("status").textContent === "Applied verified feature edit."','Publish sketch-only state');
  const sketchOnly=source();
  await review('XY','Bodyless sketch');assert.equal(source(),sketchOnly);
  assert.match(await evaluate('document.getElementById("sketch-new-effect").textContent'),/No solid yet/);
  await apply();
  checks.push('native sketch-only review and creation remains usable with null physical evidence');
  const beforeRefreshFailure=logicalSource();
  await review('XY','Refresh recovery sketch');
  await evaluate('globalThis.__creationDrawSketch=drawSketch;drawSketch=()=>{throw new Error("injected view refresh failure")}');
  try {
    await evaluate('document.getElementById("sketch-new-apply").click()');
    await until('!sketchCreateBusy && !documentMutationBusy && document.getElementById("status").textContent.startsWith("Sketch created. Refresh the document view:")','Report committed source after display failure');
    assert.equal(logicalSource()['sketches.json'].sketches.length,beforeRefreshFailure['sketches.json'].sketches.length+1);
    assert(await evaluate('documentHistoryState.can_undo'));
  } finally { await evaluate('drawSketch=globalThis.__creationDrawSketch;delete globalThis.__creationDrawSketch'); }
  await evaluate('loadDocument(currentPath,{keepParamHistory:true})');
  await until('sketchCreationState && !document.getElementById("sketch-new-review").disabled','Refresh after display failure');
  await evaluate('document.getElementById("undo-btn").click()');
  await until('!documentMutationBusy && document.getElementById("status").textContent === "Undid document edit"','Undo committed creation after display recovery');
  assert.deepEqual(logicalSource(),beforeRefreshFailure);
  checks.push('display failure reports successful backend commit and retains recoverable history');
  return checks;
}
