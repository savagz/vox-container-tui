#!/bin/sh
# Instalador de voxcontainer-tui.
#
#   curl -fsSL https://raw.githubusercontent.com/savagz/vox-container-tui/main/install.sh | sh
#
# Opciones:
#   --prefix <dir>   Directorio de instalacion (por defecto /usr/local/bin)
#   --version <tag>  Version a instalar, p. ej. v0.1.0 (por defecto la ultima)
#   --uninstall      Borra el binario instalado
#   -h, --help       Muestra esta ayuda
#
# Variables: VOXCONTAINER_VERSION equivale a --version.

set -eu

REPO="savagz/vox-container-tui"
BIN="voxcontainer-tui"
PREFIX="/usr/local/bin"
VERSION="${VOXCONTAINER_VERSION:-}"
UNINSTALL=0

info() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

usage() {
    sed -n '2,12p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//' || true
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || die "--prefix necesita un directorio"
            PREFIX="$2"; shift 2 ;;
        --version)
            [ $# -ge 2 ] || die "--version necesita un tag"
            VERSION="$2"; shift 2 ;;
        --uninstall) UNINSTALL=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) die "opcion desconocida: $1" ;;
    esac
done

# Ejecuta con sudo solo si el destino no es escribible.
as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        die "se necesitan permisos de escritura en $PREFIX (usa --prefix o instala sudo)"
    fi
}

install_file() {
    # $1 origen, $2 destino
    dest_dir=$(dirname "$2")
    if [ -d "$dest_dir" ] && [ -w "$dest_dir" ]; then
        install -m 0755 "$1" "$2"
    else
        as_root mkdir -p "$dest_dir"
        as_root install -m 0755 "$1" "$2"
    fi
}

if [ "$UNINSTALL" -eq 1 ]; then
    target="$PREFIX/$BIN"
    [ -e "$target" ] || die "no existe $target"
    if [ -w "$PREFIX" ]; then rm -f "$target"; else as_root rm -f "$target"; fi
    info "Eliminado $target"
    exit 0
fi

[ "$(uname -s)" = "Linux" ] || die "solo Linux esta soportado (detectado: $(uname -s))"
[ "$(uname -m)" = "x86_64" ] || die "solo x86_64 esta soportado (detectado: $(uname -m))"

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1" -o "$2"; }
    fetch_stdout() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO "$2" "$1"; }
    fetch_stdout() { wget -qO- "$1"; }
else
    die "se necesita curl o wget"
fi
command -v sha256sum >/dev/null 2>&1 || die "se necesita sha256sum"
command -v tar >/dev/null 2>&1 || die "se necesita tar"

if [ -z "$VERSION" ]; then
    info "Buscando la ultima release..."
    VERSION=$(fetch_stdout "https://api.github.com/repos/$REPO/releases/latest" \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1) \
        || die "no se pudo consultar la ultima release"
    [ -n "$VERSION" ] || die "no se pudo determinar la ultima release"
fi

name="$BIN-$VERSION-x86_64-linux"
base="https://github.com/$REPO/releases/download/$VERSION"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

info "Descargando $name.tar.gz..."
fetch "$base/$name.tar.gz" "$tmp/$name.tar.gz" || die "no se pudo descargar $base/$name.tar.gz"
fetch "$base/$name.tar.gz.sha256" "$tmp/$name.tar.gz.sha256" || die "no se pudo descargar el hash"

info "Verificando hash..."
(cd "$tmp" && sha256sum -c "$name.tar.gz.sha256" >/dev/null) || die "el hash no coincide, abortando"

tar -xzf "$tmp/$name.tar.gz" -C "$tmp"
[ -f "$tmp/$name/$BIN" ] || die "el paquete no contiene $BIN"

install_file "$tmp/$name/$BIN" "$PREFIX/$BIN"

info "Instalado en $PREFIX/$BIN"
"$PREFIX/$BIN" --version || true
info "La primera ejecucion abre el System Check, que revisa incus/LXD y los permisos del socket."
case ":$PATH:" in
    *":$PREFIX:"*) ;;
    *) info "Aviso: $PREFIX no esta en tu PATH." ;;
esac
