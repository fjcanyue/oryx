#!/bin/sh
# arch.sh: the Arch package, oryx-editor-bin-<version>-1-x86_64.pkg.tar.zst,
# for an install with pacman -U from the release page. Built from the
# release tarball with the oryx-editor-bin PKGBUILD, after make channels
# wrote the release's checksums into it, so makepkg verifies the tarball
# and the four files it is given against them. The four files come from
# the tag, as the PKGBUILD fetches them. Usage: arch.sh <version> <release dir>
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
refuse() { echo "arch.sh: $1" >&2; exit 1; }

[ $# -eq 2 ] || refuse "usage: arch.sh <version> <release dir>"
version=$1
out=$(cd "$2" 2>/dev/null && pwd) || refuse "$2 is not a folder"
for tool in makepkg fakeroot rsvg-convert git; do
    command -v "$tool" >/dev/null 2>&1 || refuse "$tool is missing"
done

pkgbuild="$here/aur/oryx-editor-bin/PKGBUILD"
pkgver=$(sed -n 's/^pkgver=//p' "$pkgbuild")
[ "$pkgver" = "$version" ] || refuse "the PKGBUILD is at $pkgver, not $version: run make channels first"
tarball="$out/oryx-$version-linux-x86_64.tar.gz"
[ -f "$tarball" ] || refuse "$tarball is missing: run make release first"
tag="v$version"
git -C "$root" rev-parse --verify --quiet "$tag^{commit}" >/dev/null || refuse "tag $tag is not in this checkout"

work=$(mktemp -d)
start="$work/start"
mkdir -p "$start" "$work/build"
cp "$pkgbuild" "$start/PKGBUILD"
cp "$tarball" "$start/"
# The four files under the names the PKGBUILD downloads them as, which
# makepkg takes from the build folder before fetching anything.
for pair in \
    "desktop packaging/linux/com.steerania.Oryx.desktop" \
    "metainfo.xml packaging/linux/com.steerania.Oryx.metainfo.xml" \
    "svg assets/icon/oryx.svg" \
    "stage-linux.sh packaging/stage-linux.sh"; do
    name=${pair%% *}
    path=${pair#* }
    git -C "$root" show "$tag:$path" > "$start/oryx-editor-bin-$version.$name"
done

# PKGEXT pins the standard zst package whatever this machine's makepkg
# configuration compresses with.
(cd "$start" && SRCDEST="$start" PKGDEST="$out" BUILDDIR="$work/build" LOGDEST="$work/build" \
    PKGEXT=.pkg.tar.zst makepkg -f --noconfirm)
rm -rf "$work"
package="$out/oryx-editor-bin-$version-1-x86_64.pkg.tar.zst"
[ -f "$package" ] || refuse "makepkg wrote no package into $out"
if command -v namcap >/dev/null 2>&1; then
    namcap "$package" || true
fi
(cd "$out" && sha256sum "$(basename "$package")")
