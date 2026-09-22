#!/bin/sh
# build.sh: the Mac app and its disk image. Runs on a Mac with Xcode's
# command line tools, rustup and rsvg-convert; the GitHub workflow
# calls it, and a Mac at hand can too. Usage: build.sh [output dir]
#
# The app is a universal binary (Apple Silicon and Intel joined with
# lipo) inside Oryx.app, with the themes under Contents/Resources, where
# a bundle keeps its data (codesign takes every file under MacOS for
# code) and where Oryx looks on macOS, the icon rendered from the SVG
# at the sizes an icns holds, Info.plist with the version and every
# file type Oryx opens, and an ad-hoc signature, enough to run without
# a developer account.
# The disk image carries the app and a link to Applications. The output
# folder receives the .dmg and its SHA-256 line.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
out=${1:-$root/release}
out=$(mkdir -p "$out" && cd "$out" && pwd)

for tool in cargo rustup lipo iconutil codesign hdiutil plutil rsvg-convert shasum; do
    command -v "$tool" >/dev/null 2>&1 || { echo "build.sh: $tool is missing" >&2; exit 1; }
done

version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "build.sh: no version in Cargo.toml" >&2; exit 1; }
name="oryx-$version-macos-universal.dmg"

cd "$root"
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin

stage=$(mktemp -d)
app="$stage/Oryx.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create -output "$app/Contents/MacOS/oryx" \
    target/aarch64-apple-darwin/release/oryx \
    target/x86_64-apple-darwin/release/oryx
lipo -info "$app/Contents/MacOS/oryx"
cp -R themes "$app/Contents/Resources/themes"
cp -R examples "$app/Contents/Resources/examples"
cp LICENSE "$app/Contents/Resources/LICENSE"

# The icon: each size and its @2x double, the pairs an iconset wants.
iconset="$stage/oryx.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    double=$((size * 2))
    rsvg-convert -w "$size" -h "$size" assets/icon/oryx.svg -o "$iconset/icon_${size}x${size}.png"
    rsvg-convert -w "$double" -h "$double" assets/icon/oryx.svg -o "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/oryx.icns"

sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

codesign --force --sign - "$app"
codesign --verify --strict "$app"

dmgroot="$stage/dmg"
mkdir -p "$dmgroot"
cp -R "$app" "$dmgroot/Oryx.app"
ln -s /Applications "$dmgroot/Applications"
rm -f "$out/$name"
hdiutil create -volname "Oryx $version" -srcfolder "$dmgroot" -ov -format UDZO "$out/$name"
(cd "$out" && shasum -a 256 "$name" > "$name.sha256" && cat "$name.sha256")
rm -rf "$stage"
echo "$out/$name"
