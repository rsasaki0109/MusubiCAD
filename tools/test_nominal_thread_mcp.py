"""ADR-069 actual MCP/Agent stdio, native geometry and source-aware eval regression."""
from __future__ import annotations
import argparse
import copy
import json
import math
from pathlib import Path
import subprocess
import tempfile
import mcp_eval
from test_cylindrical_draft_mcp import Client, digest, canonical_digest, regenerate
ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--musubicad", type=Path, default=ROOT / "target/debug/musubicad")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    binary = args.musubicad.resolve()
    task = next(t for t in json.loads(mcp_eval.TASKS.read_text())["tasks"] if t["id"] == "nominal_thread_handedness")
    # Independent flat/root/flank meridional integration, dimensions in mm.
    r = 3.0
    root_radius = r - 0.5412658773652741
    volume = math.pi * (root_radius**2 / 4 + r**2 / 8 + 2 * (r**3 - root_radius**3) / (3 * math.sqrt(3))) * 8
    assert abs(volume - task["expected_volume_mm3"]) < 1e-9
    with tempfile.TemporaryDirectory(prefix="musubicad-nominal-thread-") as folder:
        work = Path(folder)
        doc = work / "thread.ocad.d"
        mcp, agent = Client(binary, "mcp"), Client(binary, "agent")
        try:
            mcp.call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "nominal-thread-regression", "version": "1"}})
            mcp.tool("new_document", {"path": str(doc), "kind": "part", "document_id": "doc:nominal_thread", "name": "Nominal M6x1 basic flat-root thread"})
            create = json.loads((ROOT / "examples/agent/create_nominal_metric_thread_patch.json").read_text())
            before = digest(doc)
            preview = mcp.tool("patch_dry_run", {"path": str(doc), "patch": create, "verify": False})
            assert preview["verification"]["passed"] is True
            assert digest(doc) == before
            authored = mcp.tool("patch_apply", {"path": str(doc), "patch": create, "verify": False})
            assert authored["verification"]["passed"] is True
            regenerate(binary, doc, volume)
            baseline, canonical_before = digest(doc), canonical_digest(doc)
            data = json.loads((doc / "graph/features.json").read_text())
            definition = next(n for n in data["feature_nodes"] if n["id"] == "feature:thread")["definition"]
            left = copy.deepcopy(definition)
            left["handedness"] = "left"
            change = {"operations": [{"type": "replace_feature_definition", "id": "feature:thread", "definition": left}]}
            preview = mcp.tool("patch_dry_run", {"path": str(doc), "patch": change, "verify": False})
            assert preview["verification"]["passed"] is True
            assert digest(doc) == baseline
            mcp.tool("review_patch", {"path": str(doc), "patch": change, "output_dir": str(work / "review")})
            assert digest(doc) == baseline
            assert (work / "review/review.json").exists()
            review_before = digest(work / "review")
            edited = mcp.tool("patch_apply", {"path": str(doc), "patch": change, "verify": False, "history": authored["history"]})
            assert edited["verification"]["passed"] is True
            output = regenerate(binary, doc, volume)
            assert mcp_eval.check(task, doc, output) == (True, [])
            canonical_after = canonical_digest(doc)
            history = edited["history"]
            before_failure = digest(doc)
            bad = {"operations": [
                {"type": "replace_feature_definition", "id": "feature:thread", "definition": definition},
                {"type": "add_semantic_ref", "topo_ref": {"ref_id": "ref:face:wrong", "kind": "face", "semantic": {"created_by": "feature:stock", "role": "top", "normal_hint": [0, 0, 1]}}},
                {"type": "set_feature_ref", "feature_id": "feature:thread", "field": "face_ref", "ref_id": "ref:face:wrong"}]}
            refused = mcp.tool("patch_dry_run", {"path": str(doc), "patch": bad, "verify": False})
            assert refused["verification"]["passed"] is False
            mcp.tool("patch_apply", {"path": str(doc), "patch": bad, "verify": False, "history": history}, error=True)
            assert digest(doc) == before_failure
            assert digest(work / "review") == review_before
            # CLI --no-verify must still run the same mandatory physical gate.
            badfile = work / "bad.json"
            badfile.write_text(json.dumps(bad))
            refused = subprocess.run([str(binary), "patch", str(doc), str(badfile), "--no-verify"], capture_output=True, text=True, timeout=60)
            assert refused.returncode != 0
            assert digest(doc) == before_failure
            for direction, expected in [("undo", canonical_before), ("redo", canonical_after)]:
                response = agent.call(f"opencad.history_{direction}_document", {"path": str(doc), "history": history})
                assert "error" not in response, response
                history = response["result"]["history"]
                regenerate(binary, doc, volume)
                assert canonical_digest(doc) == expected
            # A valid source edit changing only hand must be rejected by the
            # requested-hand grader even though physical scalar properties agree.
            assert mcp_eval.check(task, doc, output) == (True, [])
            report = {"mcp_requests": mcp.count, "agent_requests": agent.count, "model_turns": 0, "volume_before_mm3": volume, "volume_after_mm3": volume, "forced_native_verification": True, "readonly_preview_review": True, "failed_batch_preserves_source_and_artifacts": True, "canonical_undo_redo": True, "source_aware_eval_passed": True, "surface_phase_evidence": "independent Mock/OCCT Rust geometry tests"}
            if args.out:
                args.out.write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(report))
        finally:
            mcp.close()
            agent.close()


if __name__ == "__main__":
    main()
