"""Verify the agent plugin layout and the release installer.

Checks that every agent host manifest launches the same `musubicad mcp`
server and ships the skills, that each skill has valid frontmatter, and runs
install.sh end to end against a local release served over HTTP: a verified
install, a checksum mismatch, and an archive without the `musubicad`
executable.
"""

from __future__ import annotations

import functools
import hashlib
import http.server
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import threading


ROOT = Path(__file__).resolve().parents[1]
MCP_SERVER = {"musubicad": {"command": "musubicad", "args": ["mcp"]}}
PLUGIN_MANIFESTS = (
    ROOT / ".claude-plugin" / "plugin.json",
    ROOT / ".codex-plugin" / "plugin.json",
    ROOT / ".cursor-plugin" / "plugin.json",
)


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def check(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def skill_frontmatter(path: Path) -> dict[str, str]:
    text = path.read_text(encoding="utf-8")
    match = re.match(r"^---\n(.*?)\n---\n", text, re.DOTALL)
    check(match is not None, f"{path}: missing YAML frontmatter")
    fields = {}
    for line in match.group(1).splitlines():
        key, _, value = line.partition(":")
        fields[key.strip()] = value.strip()
    return fields


def verify_manifests() -> None:
    skills = sorted(path.parent.name for path in (ROOT / "skills").glob("*/SKILL.md"))
    check(skills == ["musubicad"], f"unexpected skills: {skills}")
    for name in skills:
        fields = skill_frontmatter(ROOT / "skills" / name / "SKILL.md")
        check(fields.get("name") == name, f"skill {name}: frontmatter name is {fields.get('name')!r}")
        description = fields.get("description", "")
        check(0 < len(description) <= 1024, f"skill {name}: description must be 1-1024 characters")

    for path in PLUGIN_MANIFESTS:
        manifest = load(path)
        check(manifest["name"] == "musubicad", f"{path}: name")
        check(manifest["skills"] == "./skills/", f"{path}: skills")
        mcp_path = ROOT / manifest["mcpServers"]
        check(mcp_path.is_file(), f"{path}: missing {manifest['mcpServers']}")
        check(load(mcp_path) == {"mcpServers": MCP_SERVER}, f"{mcp_path}: server")

    marketplace = load(ROOT / ".claude-plugin" / "marketplace.json")
    check([plugin["name"] for plugin in marketplace["plugins"]] == ["musubicad"], "marketplace plugins")
    check(marketplace["plugins"][0]["source"] == "./", "marketplace source")

    gemini = load(ROOT / "gemini-extension.json")
    check(gemini["mcpServers"] == MCP_SERVER, "gemini-extension.json: server")
    check((ROOT / gemini["contextFileName"]).is_file(), "gemini-extension.json: contextFileName")

    listed = [skill for group in load(ROOT / "skills.sh.json")["groupings"] for skill in group["skills"]]
    check(sorted(listed) == skills, f"skills.sh.json lists {listed}, skills are {skills}")


def platform() -> str:
    system = {"Linux": "linux", "Darwin": "macos"}[os.uname().sysname]
    machine = {"x86_64": "x86_64", "amd64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}[
        os.uname().machine
    ]
    return f"{system}-{machine}"


def fake_release(directory: Path, tag: str, executable: str, tamper: bool = False) -> None:
    """Write download/<tag>/<archive> and SHA256SUMS like the release workflow."""
    package = f"musubicad-cli-{tag}-{platform()}"
    script = f"#!/bin/sh\necho 'musubicad {tag[1:]}'\necho 'OCCT test'\n".encode()
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        info = tarfile.TarInfo(f"{package}/{executable}")
        info.size = len(script)
        info.mode = 0o755
        archive.addfile(info, io.BytesIO(script))
    data = buffer.getvalue()
    release = directory / "download" / tag
    release.mkdir(parents=True)
    (release / f"{package}.tar.gz").write_bytes(data)
    digest = hashlib.sha256(data).hexdigest()
    if tamper:
        digest = ("0" if digest[0] != "0" else "1") + digest[1:]
    (release / "SHA256SUMS").write_text(f"{digest}  {package}.tar.gz\n", encoding="ascii")


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass


def run_installer(base_url: str, tag: str, install_dir: Path) -> subprocess.CompletedProcess:
    env = {
        "PATH": os.environ["PATH"],
        "HOME": str(install_dir.parent),
        "MUSUBICAD_BASE_URL": base_url,
        "MUSUBICAD_VERSION": tag,
        "MUSUBICAD_INSTALL_DIR": str(install_dir),
        "NO_PROXY": "127.0.0.1,localhost",
        "no_proxy": "127.0.0.1,localhost",
    }
    return subprocess.run(
        ["sh", str(ROOT / "install.sh")], env=env, capture_output=True, text=True, timeout=120
    )


def verify_installer() -> None:
    with tempfile.TemporaryDirectory() as temp:
        serve_root = Path(temp) / "releases"
        fake_release(serve_root, "v9.0.0", "musubicad")
        fake_release(serve_root, "v9.0.1", "musubicad", tamper=True)
        fake_release(serve_root, "v0.1.1", "opencad")
        handler = functools.partial(QuietHandler, directory=str(serve_root))
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        base_url = f"http://127.0.0.1:{server.server_address[1]}"
        try:
            ok_dir = Path(temp) / "ok" / "bin"
            result = run_installer(base_url, "9.0.0", ok_dir)
            check(result.returncode == 0, f"install failed: {result.stderr}")
            check("installed musubicad 9.0.0" in result.stderr, result.stderr)
            check(os.access(ok_dir / "musubicad", os.X_OK), "installed binary is not executable")

            bad_dir = Path(temp) / "bad" / "bin"
            result = run_installer(base_url, "v9.0.1", bad_dir)
            check(result.returncode != 0, "tampered archive was installed")
            check("checksum mismatch" in result.stderr, result.stderr)
            check(not (bad_dir / "musubicad").exists(), "tampered binary was written")

            old_dir = Path(temp) / "old" / "bin"
            result = run_installer(base_url, "0.1.1", old_dir)
            check(result.returncode != 0, "archive without musubicad was installed")
            check("predates the musubicad command" in result.stderr, result.stderr)
        finally:
            server.shutdown()


def main() -> int:
    try:
        verify_manifests()
        verify_installer()
    except (AssertionError, KeyError, OSError, json.JSONDecodeError) as error:
        print(f"agent plugin contract failed: {error}", file=sys.stderr)
        return 1
    print("agent plugin contract: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
