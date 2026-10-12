"""Reject generated Python bytecode in the source index (MCAD-P13-009)."""
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    tracked = subprocess.check_output(
        ["git", "ls-files", "-z"], cwd=ROOT
    ).decode().split("\0")
    cached = [
        name for name in tracked
        if "__pycache__" in Path(name).parts or name.endswith((".pyc", ".pyo"))
    ]
    assert not cached, f"generated Python cache is tracked: {cached}"
    for name in ("tools/__pycache__/probe.cpython-312.pyc", "tools/probe.pyc", "tools/probe.pyo"):
        result = subprocess.run(
            ["git", "check-ignore", "--no-index", "-q", name], cwd=ROOT, check=False
        )
        assert result.returncode == 0, f"generated Python cache is not ignored: {name}"
    print("repository Python cache hygiene: ok")


if __name__ == "__main__":
    main()
