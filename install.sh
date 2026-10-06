#!/bin/sh
# Install the MusubiCAD CLI (`musubicad`) from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/rsasaki0109/MusubiCAD/main/install.sh | sh
#
# Downloads the release archive for this machine, verifies it against the
# release's SHA256SUMS, and installs one self-contained executable (OpenCASCADE
# is linked in).  Nothing else is installed and no build is needed.
#
# Environment:
#   MUSUBICAD_VERSION      release to install, e.g. 0.2.0 (default: latest)
#   MUSUBICAD_INSTALL_DIR  destination directory (default: $HOME/.local/bin)
#   MUSUBICAD_REPO         GitHub owner/repo (default: rsasaki0109/MusubiCAD)
#   MUSUBICAD_BASE_URL     releases URL (default: https://github.com/$MUSUBICAD_REPO/releases)
set -eu

REPO="${MUSUBICAD_REPO:-rsasaki0109/MusubiCAD}"
VERSION="${MUSUBICAD_VERSION:-latest}"
INSTALL_DIR="${MUSUBICAD_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="${MUSUBICAD_BASE_URL:-https://github.com/$REPO/releases}"

say() { printf 'musubicad-install: %s\n' "$*" >&2; }
fail() {
  say "error: $*"
  exit 1
}

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$2" "$1" || fail "download failed: $1"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$2" "$1" || fail "download failed: $1"
  else
    fail "curl or wget is required"
  fi
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  else
    fail "sha256sum or shasum is required to verify the download"
  fi
}

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) fail "unsupported OS '$(uname -s)'; on Windows run install.ps1 in PowerShell" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) arch="$(uname -m)" ;;
esac
platform="$os-$arch"
case "$platform" in
  linux-x86_64 | macos-aarch64 | macos-x86_64) ;;
  *) fail "no prebuilt binary for $platform; build from source: https://github.com/$REPO/blob/main/docs/developer-guide/index.md" ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

if [ "$VERSION" = latest ]; then
  fetch "https://api.github.com/repos/$REPO/releases/latest" "$tmp/latest.json"
  tag="$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$tmp/latest.json" | head -n 1)"
  [ -n "$tag" ] || fail "could not determine the latest release of $REPO"
else
  tag="v${VERSION#v}"
fi

package="musubicad-cli-$tag-$platform"
archive="$package.tar.gz"
say "downloading $archive"
fetch "$BASE_URL/download/$tag/$archive" "$tmp/$archive"
fetch "$BASE_URL/download/$tag/SHA256SUMS" "$tmp/SHA256SUMS"

expected="$(awk -v name="$archive" '$2 == name || $2 == "*" name { print $1 }' "$tmp/SHA256SUMS")"
[ -n "$expected" ] || fail "SHA256SUMS of $tag has no entry for $archive"
actual="$(sha256 "$tmp/$archive")"
[ "$expected" = "$actual" ] || fail "checksum mismatch for $archive (expected $expected, got $actual)"

tar -xzf "$tmp/$archive" -C "$tmp"
binary="$tmp/$package/musubicad"
[ -f "$binary" ] || fail "$tag predates the musubicad command; install v0.2.0 or newer"

mkdir -p "$INSTALL_DIR"
staged="$INSTALL_DIR/.musubicad.$$"
cp "$binary" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$INSTALL_DIR/musubicad"

installed="$("$INSTALL_DIR/musubicad" version | head -n 1)"
say "installed $installed to $INSTALL_DIR/musubicad"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) say "add it to PATH, e.g. in your shell profile: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac

cat >&2 <<EOF
musubicad-install: next, connect your agent:
  Claude Code:  claude plugin marketplace add $REPO && claude plugin install musubicad@musubicad
  Any MCP host: command "musubicad", args ["mcp"]
EOF
