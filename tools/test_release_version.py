"""Verify the shared CLI and desktop release version contract."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[1]
SEMVER = re.compile(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$")


def read_toml(path: Path) -> dict:
    with path.open("rb") as source:
        return tomllib.load(source)


def require_version(actual: object, expected: str, source: Path) -> None:
    if actual != expected:
        raise AssertionError(f"{source}: expected version {expected!r}, found {actual!r}")


def verify_lock(path: Path, expected_packages: set[str], version: str) -> None:
    lock = read_toml(path)
    entries = {
        (package.get("name"), package.get("version"))
        for package in lock.get("package", [])
    }
    for name in sorted(expected_packages):
        if (name, version) not in entries:
            raise AssertionError(f"{path}: missing {name} {version}")


def verify(root: Path, tag: str | None) -> str:
    root_manifest_path = root / "Cargo.toml"
    root_manifest = read_toml(root_manifest_path)
    version = root_manifest["workspace"]["package"]["version"]
    if not isinstance(version, str) or SEMVER.fullmatch(version) is None:
        raise AssertionError(
            f"{root_manifest_path}: release version must be stable MAJOR.MINOR.PATCH"
        )

    workspace_packages: set[str] = set()
    for member in root_manifest["workspace"]["members"]:
        manifest_path = root / member / "Cargo.toml"
        manifest = read_toml(manifest_path)
        package = manifest["package"]
        workspace_packages.add(package["name"])
        declared = package.get("version")
        if declared != {"workspace": True}:
            require_version(declared, version, manifest_path)

    verify_lock(root / "Cargo.lock", workspace_packages, version)

    desktop_root = root / "apps" / "desktop" / "src-tauri"
    desktop_manifest_path = desktop_root / "Cargo.toml"
    desktop_manifest = read_toml(desktop_manifest_path)
    desktop_name = desktop_manifest["package"]["name"]
    require_version(desktop_manifest["package"].get("version"), version, desktop_manifest_path)

    tauri_config_path = desktop_root / "tauri.conf.json"
    tauri_config = json.loads(tauri_config_path.read_text(encoding="utf-8"))
    require_version(tauri_config.get("version"), version, tauri_config_path)

    verify_lock(desktop_root / "Cargo.lock", {desktop_name}, version)
    desktop_lock = read_toml(desktop_root / "Cargo.lock")
    desktop_workspace_packages = {
        package["name"]
        for package in desktop_lock.get("package", [])
        if package.get("name") in workspace_packages
    }
    verify_lock(desktop_root / "Cargo.lock", desktop_workspace_packages, version)

    # Agent plugin manifests ship from the same tag as the CLI they launch.
    for manifest_path in (
        root / ".claude-plugin" / "plugin.json",
        root / ".codex-plugin" / "plugin.json",
        root / ".cursor-plugin" / "plugin.json",
        root / "gemini-extension.json",
    ):
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        require_version(manifest.get("version"), version, manifest_path)
    marketplace_path = root / ".claude-plugin" / "marketplace.json"
    marketplace = json.loads(marketplace_path.read_text(encoding="utf-8"))
    for plugin in marketplace["plugins"]:
        require_version(plugin.get("version"), version, marketplace_path)

    if tag is not None and tag != f"v{version}":
        raise AssertionError(f"tag {tag!r} does not match release version v{version}")
    return version


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", help="optional v<version> tag to verify")
    args = parser.parse_args()
    try:
        version = verify(ROOT, args.tag)
    except (AssertionError, KeyError, OSError, tomllib.TOMLDecodeError, json.JSONDecodeError) as error:
        print(f"release version contract failed: {error}", file=sys.stderr)
        return 1
    print(f"release version contract: v{version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
