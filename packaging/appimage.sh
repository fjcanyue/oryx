#!/bin/sh
# Packs the staged Linux tree as an AppImage with appimagetool.
# Usage: appimage.sh <staged usr dir> <output.AppImage>
#
# The AppDir holds the tree under usr/, AppRun from packaging/linux/,
# and at the top, where the runtime and the desktop integration tools
# look for them, the desktop entry and the 256 px icon with .DirIcon
# linked to it. The binary is the portable one build-linux.sh produces.
# Bundled with it under usr/lib: the three libraries of the X11 keyboard
# path (libxkbcommon, libxkbcommon-x11, libxcb-xkb), which winit loads at
# run time and which a minimal system lacks (the test machine of the
# AppImage catalog, 04/10/2026). They come from the Ubuntu 22.04 root,
# or from the folder APPIMAGE_LIBS names, and the binary gets a RUNPATH
# to them with patchelf, so that nothing leaks into the programs Oryx
# starts. The image still needs the system's glibc 2.35 and libssl 3,
# the floor of the tarball.
#
# appimagetool downloads the AppImage runtime from GitHub on every run
# unless given one. A copy kept at ~/.local/lib/appimagetool/runtime-x86_64
# (or the file APPIMAGE_RUNTIME names) is used when present, so a release
# builds without the network and every build embeds the same runtime.
# The tool's own AppStream check is skipped (-n): it only recognizes the
# old .appdata.xml name, and tests/packaging.rs validates the metainfo.
set -e
here="$(cd "$(dirname "$0")" && pwd)"
STAGE=$1
OUT=$2
if [ -z "$STAGE" ] || [ -z "$OUT" ]; then
    echo "usage: appimage.sh <staged usr dir> <output.AppImage>" >&2
    exit 1
fi
for tool in appimagetool patchelf; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "appimage.sh: $tool is not installed" >&2
        exit 1
    fi
done
libs="${APPIMAGE_LIBS:-/var/lib/oryx-jammy/usr/lib/x86_64-linux-gnu}"
keyboard="libxkbcommon.so.0 libxkbcommon-x11.so.0 libxcb-xkb.so.1"
app=com.steerania.Oryx
apprun="$here/linux/AppRun"
entry="$STAGE/share/applications/$app.desktop"
icon="$STAGE/share/icons/hicolor/256x256/apps/$app.png"
for file in "$STAGE/bin/oryx" "$entry" "$icon" "$apprun"; do
    if [ ! -e "$file" ]; then
        echo "appimage.sh: $file is missing" >&2
        exit 1
    fi
done
for lib in $keyboard; do
    if [ ! -e "$libs/$lib" ]; then
        echo "appimage.sh: $libs/$lib is missing" >&2
        exit 1
    fi
done

appdir=$(mktemp -d)
trap 'rm -rf "$appdir"' EXIT
# mktemp gives the folder to its owner alone, and its mode becomes the
# mode of the top folder of the image. firejail mounts the image as a
# plain user, who must be able to enter it.
chmod 755 "$appdir"
cp -r "$STAGE" "$appdir/usr"
# The soname of each library is a link in the root; the copy is the
# file itself under that name. A RUNPATH reaches the direct needs of
# the file that carries it alone: the binary's leads the loader to the
# folder for the two libraries it opens, and each library's leads it
# there for the library's own needs (libxkbcommon-x11 needs
# libxkbcommon and libxcb-xkb).
mkdir -p "$appdir/usr/lib"
for lib in $keyboard; do
    cp -L "$libs/$lib" "$appdir/usr/lib/$lib"
    chmod 644 "$appdir/usr/lib/$lib"
    patchelf --set-rpath '$ORIGIN' "$appdir/usr/lib/$lib"
done
patchelf --set-rpath '$ORIGIN/../lib' "$appdir/usr/bin/oryx"
install -m755 "$apprun" "$appdir/AppRun"
cp "$entry" "$appdir/$app.desktop"
cp "$icon" "$appdir/$app.png"
ln -s "$app.png" "$appdir/.DirIcon"

runtime="${APPIMAGE_RUNTIME:-$HOME/.local/lib/appimagetool/runtime-x86_64}"
rm -f "$OUT"
if [ -f "$runtime" ]; then
    ARCH=x86_64 appimagetool -n --runtime-file "$runtime" "$appdir" "$OUT"
else
    echo "appimage.sh: no runtime at $runtime, appimagetool downloads one" >&2
    ARCH=x86_64 appimagetool -n "$appdir" "$OUT"
fi
echo "$OUT: $(($(stat -c %s "$OUT") / 1024)) KB"
