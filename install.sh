#!/bin/sh
# voxcontainer-tui installer.
#
#   curl -fsSL https://raw.githubusercontent.com/savagz/vox-container-tui/main/install.sh | sh
#
# Options:
#   --prefix <dir>   Install directory (default /usr/local/bin)
#   --version <tag>  Version to install, e.g. v0.1.0 (default: latest)
#   --uninstall      Remove the installed binary
#   -h, --help       Show this help
#
# Environment: VOXCONTAINER_VERSION is equivalent to --version.

set -eu

REPO="savagz/vox-container-tui"
BIN="voxcontainer-tui"
PREFIX="/usr/local/bin"
VERSION="${VOXCONTAINER_VERSION:-}"
UNINSTALL=0

info() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() {
    # Inline text: under `curl | sh`, $0 is the shell, not this file.
    cat <<EOF
voxcontainer-tui installer.

  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | sh

Options:
  --prefix <dir>   Install directory (default /usr/local/bin)
  --version <tag>  Version to install, e.g. v0.1.0 (default: latest)
  --uninstall      Remove the installed binary
  -h, --help       Show this help

Environment: VOXCONTAINER_VERSION is equivalent to --version.
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || die "--prefix requires a directory"
            PREFIX="$2"; shift 2 ;;
        --version)
            [ $# -ge 2 ] || die "--version requires a tag"
            VERSION="$2"; shift 2 ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done

# Run with sudo only if the destination is not writable.
as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        die "write permission on $PREFIX is required (use --prefix or install sudo)"
    fi
}

install_file() {
    # $1 source, $2 destination
    dest_dir=$(dirname "$2")
    # Try to create the directory without root first (e.g. ~/.local/bin).
    mkdir -p "$dest_dir" 2>/dev/null || true
    if [ -d "$dest_dir" ] && [ -w "$dest_dir" ]; then
        install -m 0755 "$1" "$2"
    else
        as_root mkdir -p "$dest_dir"
        as_root install -m 0755 "$1" "$2"
    fi
}

if [ "$UNINSTALL" -eq 1 ]; then
    target="$PREFIX/$BIN"
    [ -e "$target" ] || die "$target does not exist"
    if [ -w "$PREFIX" ]; then rm -f "$target"; else as_root rm -f "$target"; fi
    info "Removed $target"
    exit 0
fi

[ "$(uname -s)" = "Linux" ] || die "only Linux is supported (detected: $(uname -s))"
[ "$(uname -m)" = "x86_64" ] || die "only x86_64 is supported (detected: $(uname -m))"

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1" -o "$2"; }
    fetch_stdout() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO "$2" "$1"; }
    fetch_stdout() { wget -qO- "$1"; }
else
    die "curl or wget is required"
fi
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required"
command -v tar >/dev/null 2>&1 || die "tar is required"

if [ -z "$VERSION" ]; then
    info "Looking up the latest release..."
    VERSION=$(fetch_stdout "https://api.github.com/repos/$REPO/releases/latest" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1) \
        || die "could not query the latest release"
    [ -n "$VERSION" ] || die "could not determine the latest release"
fi

# Accept "1.2.3" as well as "v1.2.3".
case "$VERSION" in
    v*) ;;
    *) VERSION="v$VERSION" ;;
esac

name="$BIN-$VERSION-x86_64-linux"
base="https://github.com/$REPO/releases/download/$VERSION"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
trap 'exit 130' INT TERM

info "Downloading $name.tar.gz..."
fetch "$base/$name.tar.gz" "$tmp/$name.tar.gz" || die "could not download $base/$name.tar.gz"
fetch "$base/$name.tar.gz.sha256" "$tmp/$name.tar.gz.sha256" || die "could not download the checksum"

info "Verifying checksum..."
(cd "$tmp" && sha256sum -c "$name.tar.gz.sha256" >/dev/null) || die "checksum mismatch, aborting"

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
[ -f "$tmp/$name/$BIN" ] || die "the package does not contain $BIN"

install_file "$tmp/$name/$BIN" "$PREFIX/$BIN"

info "Installed to $PREFIX/$BIN"
"$PREFIX/$BIN" --version || true
info "The first run opens the System Check, which verifies incus/LXD and the socket permissions."
case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *) info "Warning: $PREFIX is not in your PATH." ;;
esac
