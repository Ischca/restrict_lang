#!/usr/bin/env bash
set -euo pipefail

VERSION="${RESTRICT_LANG_VERSION:-0.0.1}"
VERSION="${VERSION#v}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.restrict-lang}"
RELEASE_ROOT="https://github.com/Ischca/restrict_lang/releases/download/v${VERSION}"

fail() {
    echo "error: $*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "$1 is required"
}

case "$(uname -s)" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *) fail "the shell installer supports Linux and macOS; use the Windows release archive directly" ;;
esac

case "$(uname -m)" in
    x86_64 | amd64) arch="x86_64" ;;
    arm64 | aarch64) arch="aarch64" ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
esac

if [ "$os" = "linux" ] && [ "$arch" != "x86_64" ]; then
    fail "v${VERSION} does not publish a Linux ${arch} archive"
fi

require_command curl
require_command tar

platform="${os}-${arch}"
package="restrict-lang-v${VERSION}-${platform}"
archive="${package}.tar.gz"
temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT

echo "Downloading Restrict Language v${VERSION} for ${platform}..."
curl --fail --location --silent --show-error "${RELEASE_ROOT}/${archive}" --output "${temp_dir}/${archive}"
curl --fail --location --silent --show-error "${RELEASE_ROOT}/SHA256SUMS" --output "${temp_dir}/SHA256SUMS"

cd "$temp_dir"
checksum_line="$(grep "  ${archive}$" SHA256SUMS || true)"
[ -n "$checksum_line" ] || fail "${archive} is missing from SHA256SUMS"

if command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' "$checksum_line" | sha256sum -c -
elif command -v shasum >/dev/null 2>&1; then
    printf '%s\n' "$checksum_line" | shasum -a 256 -c -
else
    fail "sha256sum or shasum is required to verify the download"
fi

tar -xzf "$archive"
mkdir -p "$INSTALL_DIR/bin"
install -m 0755 "$package/restrict_lang" "$INSTALL_DIR/bin/restrict_lang"
install -m 0755 "$package/warder" "$INSTALL_DIR/bin/warder"

echo "Installed Restrict Language v${VERSION} in $INSTALL_DIR/bin"
echo "Add $INSTALL_DIR/bin to PATH, then run: restrict_lang --version"
