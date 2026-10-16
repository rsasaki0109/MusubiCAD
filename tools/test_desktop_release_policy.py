"""Contract tests for the unsigned and credential-gated desktop workflows.

This is intentionally a small dependency-free test.  PyYAML is used when it
is available (for syntax and trigger shape); the textual assertions still run
on a clean runner without third-party Python packages.
"""

from __future__ import annotations

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / ".github" / "workflows" / "desktop.yml"
SIGNED = ROOT / ".github" / "workflows" / "desktop-signed-release.yml"
BUNDLE_CLEANUP = ROOT / "tools" / "clear-desktop-bundle.ps1"
RELEASE = ROOT / ".github" / "workflows" / "release.yml"


def require(text: str, needle: str, source: Path) -> None:
    if needle not in text:
        raise AssertionError(f"{source}: missing {needle!r}")


def parse_yaml_if_available(text: str, source: Path):
    try:
        import yaml  # type: ignore[import-not-found]
    except ModuleNotFoundError:
        return None

    # PyYAML follows YAML 1.1 by default and treats the GitHub key `on` as a
    # boolean. Remove only that implicit resolver so the GitHub workflow shape
    # can be checked without changing the repository's YAML.
    class GithubLoader(yaml.SafeLoader):
        pass

    for first_character, resolvers in list(GithubLoader.yaml_implicit_resolvers.items()):
        GithubLoader.yaml_implicit_resolvers[first_character] = [
            (tag, regexp)
            for tag, regexp in resolvers
            if tag != "tag:yaml.org,2002:bool"
        ]
    try:
        return yaml.load(text, Loader=GithubLoader)
    except yaml.YAMLError as exc:
        raise AssertionError(f"{source}: invalid YAML: {exc}") from exc


def test_yaml_shape() -> None:
    desktop_text = DESKTOP.read_text(encoding="utf-8")
    signed_text = SIGNED.read_text(encoding="utf-8")
    desktop = parse_yaml_if_available(desktop_text, DESKTOP)
    signed = parse_yaml_if_available(signed_text, SIGNED)
    if desktop is not None:
        assert isinstance(desktop, dict)
        assert desktop["on"]["pull_request"]
        assert desktop["permissions"] == {"contents": "read"}
    if signed is not None:
        assert isinstance(signed, dict)
        assert set(signed["on"]) == {"workflow_run", "workflow_dispatch"}
        assert signed["on"]["workflow_run"]["workflows"] == ["Release"]
        assert signed["on"]["workflow_run"]["types"] == ["completed"]
        assert str(signed["on"]["workflow_dispatch"]["inputs"]["tag"]["required"]).lower() == "true"
        assert signed["jobs"]["publish"]["permissions"] == {"contents": "write"}
        assert signed["jobs"]["prepare"]["permissions"] == {"contents": "read"}
        assert signed["jobs"]["signed-build"]["permissions"] == {"contents": "read"}


def test_unsigned_workflow_has_no_signing_boundary() -> None:
    text = DESKTOP.read_text(encoding="utf-8")
    require(text, "permissions:\n  contents: read", DESKTOP)
    if re.search(r"secrets\.|signtool|notarytool|codesign|contents: write", text, re.I):
        raise AssertionError(f"{DESKTOP}: unsigned workflow contains a signing or write boundary")


def test_tauri_cli_install_is_cache_safe_and_version_pinned() -> None:
    for source in (DESKTOP, SIGNED):
        text = source.read_text(encoding="utf-8")
        for needle in (
            'expected="tauri-cli $TAURI_CLI_VERSION"',
            'actual="$(cargo tauri --version 2>/dev/null || true)"',
            'cargo install tauri-cli --version "$TAURI_CLI_VERSION" --locked --force',
            'test "$(cargo tauri --version)" = "$expected"',
        ):
            require(text, needle, source)


def test_cached_bundle_cleanup_is_narrow_and_shared() -> None:
    cleanup = BUNDLE_CLEANUP.read_text(encoding="utf-8")
    for needle in (
        "apps/desktop/src-tauri/target",
        'GetFileName($Bundle) -ne "bundle"',
        'GetFileName([System.IO.Path]::GetDirectoryName($Bundle)) -ne "release"',
        "Remove-Item -LiteralPath $Bundle -Recurse -Force",
    ):
        require(cleanup, needle, BUNDLE_CLEANUP)
    for source in (DESKTOP, SIGNED):
        text = source.read_text(encoding="utf-8")
        require(text, "Clear cached desktop bundle outputs", source)
        require(text, "./tools/clear-desktop-bundle.ps1", source)
        require(text, '-BundleDirectory "${{ matrix.bundle_directory }}"', source)


def test_signed_workflow_contract() -> None:
    text = SIGNED.read_text(encoding="utf-8")
    for needle in (
        "workflow_dispatch:",
        "workflow_run:",
        "github.event.workflow_run.conclusion == 'success'",
        "github.event.workflow_run.event == 'push'",
        "desktop-release",
        "WINDOWS_CERTIFICATE",
        "WINDOWS_CERTIFICATE_PASSWORD",
        "WINDOWS_SIGNING_THUMBPRINT",
        "WINDOWS_TIMESTAMP_URL",
        "APPLE_CERTIFICATE",
        "APPLE_CERTIFICATE_PASSWORD",
        "KEYCHAIN_PASSWORD",
        "APPLE_ID",
        "APPLE_PASSWORD",
        "APPLE_TEAM_ID",
        'ref: ${{ needs.prepare.outputs.sha }}',
        'test "$(git rev-parse HEAD)" = "$EXPECTED_SHA"',
        'Release workflow SHA does not match tag',
        "Get-AuthenticodeSignature",
        "signtool.exe",
        "codesign --verify",
        "xcrun notarytool submit",
        "xcrun stapler validate",
        "sha256sum --check SHA256SUMS",
        "gh release upload",
        "The CLI release workflow must create release",
        "Remove imported Windows signing material",
        "Remove imported Apple signing material",
        "actual_authority",
        "actual_team",
        "MUSUBICAD-DESKTOP-SHA256SUMS",
        "contents: write",
    ):
        require(text, needle, SIGNED)
    if "secrets: inherit" in text:
        raise AssertionError(f"{SIGNED}: broad secret inheritance is forbidden")
    if re.search(r"^\s+pull_request\s*:", text, re.MULTILINE):
        raise AssertionError(f"{SIGNED}: signed workflow must not have a pull_request trigger")
    if re.search(r"^\s+workflow_call\s*:", text, re.MULTILINE):
        raise AssertionError(f"{SIGNED}: signed workflow must not expose a reusable-workflow trigger")
    if "--clobber" in text or "gh release create" in text:
        raise AssertionError(f"{SIGNED}: desktop publication must not replace assets or create the shared release")
    mutable_action = re.search(r"^\s*uses:\s+[^\s]+@(?![0-9a-f]{40}(?:\s|$))", text, re.MULTILINE)
    if mutable_action:
        raise AssertionError(f"{SIGNED}: release action is not pinned to a commit: {mutable_action.group(0)!r}")
    require(text, "Linux artifacts are not code-signed or notarized", SIGNED)
    require(text, "Tag/version mismatch", SIGNED)
    require(text, "tag_sha", SIGNED)


def test_signed_credentials_fail_before_expensive_setup() -> None:
    text = SIGNED.read_text(encoding="utf-8")
    immutable = text.index("- name: Verify immutable build commit")
    windows_gate = text.index(
        "- name: Fail closed when Windows signing credentials are incomplete"
    )
    apple_gate = text.index(
        "- name: Fail closed when Apple signing/notarization credentials are incomplete"
    )
    expensive_steps = (
        "- name: Install Linux system dependencies",
        "- name: Verify Apple SDK",
        "- name: Verify MSVC toolchain",
        "- name: Install Rust toolchain",
        "- name: Cache Cargo and OCCT",
        "- name: Install pinned Tauri CLI",
    )
    assert immutable < windows_gate
    assert immutable < apple_gate
    for step in expensive_steps:
        position = text.index(step)
        assert windows_gate < position, f"Windows credential gate must precede {step}"
        assert apple_gate < position, f"Apple credential gate must precede {step}"


def test_windows_ui_is_a_required_real_app_gate() -> None:
    text = DESKTOP.read_text(encoding="utf-8")
    for needle in ('include="[$linux,$windows]"',
                   'Exercise actual Windows native Tauri forms',
                   './tools/test_windows_desktop_ui.ps1',
                   '-BinaryPath "${{ matrix.binary_path }}"',
                   "node-version: '24'"):
        require(text, needle, DESKTOP)
    parsed = parse_yaml_if_available(text, DESKTOP)
    if parsed is not None:
        assert "tools/sketch_creation_ui_checks.mjs" in parsed["on"]["pull_request"]["paths"]
        assert "tools/datum_sketch_ui_checks.mjs" in parsed["on"]["pull_request"]["paths"]
        assert "tools/pointer_circle_ui_checks.mjs" in parsed["on"]["pull_request"]["paths"]
        assert "tools/pointer_rectangle_ui_checks.mjs" in parsed["on"]["pull_request"]["paths"]
        assert "tools/pointer_arc_ui_checks.mjs" in parsed["on"]["pull_request"]["paths"]
        steps = parsed["jobs"]["build"]["steps"]
        ui = next(step for step in steps if step.get("name") == "Exercise actual Windows native Tauri forms")
        assert ui["if"] == "${{ matrix.platform == 'windows-x86_64' }}"
        assert not ui.get("continue-on-error", False)
        assert ui["shell"] == "pwsh"
        names = [step.get("name") for step in steps]
        assert names.index("Verify native executable") < names.index(ui["name"])
        assert names.index(ui["name"]) < names.index("Package desktop artifacts and generate checksums")


def test_release_pr_runs_windows_packaged_tutorial_before_upload() -> None:
    text = RELEASE.read_text(encoding="utf-8")
    require(text, 'include="[$linux,$windows]"', RELEASE)
    require(text, 'include="[$linux,$windows,$mac_arm,$mac_intel]"', RELEASE)
    parsed = parse_yaml_if_available(text, RELEASE)
    if parsed is not None:
        steps = parsed["jobs"]["build"]["steps"]
        tutorial = next(step for step in steps if step.get("name") == "Execute tutorial from the packaged native archive")
        assert not tutorial.get("if")
        assert not tutorial.get("continue-on-error", False)
        assert "run_editing_tutorial.py" in tutorial["run"]
        assert "run_assembly_tutorial.py" in tutorial["run"]
        names = [step.get("name") for step in steps]
        assert names.index("Verify archive contract") < names.index(tutorial["name"])
        assert names.index(tutorial["name"]) < names.index("Upload release archive")
        publish = parsed["jobs"]["publish"]
        assert publish["if"] == "${{ github.ref_type == 'tag' }}"
        assert publish["needs"] == "build"


def main() -> int:
    test_yaml_shape()
    test_unsigned_workflow_has_no_signing_boundary()
    test_tauri_cli_install_is_cache_safe_and_version_pinned()
    test_cached_bundle_cleanup_is_narrow_and_shared()
    test_signed_workflow_contract()
    test_signed_credentials_fail_before_expensive_setup()
    test_windows_ui_is_a_required_real_app_gate()
    test_release_pr_runs_windows_packaged_tutorial_before_upload()
    print("desktop release policy contract: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
