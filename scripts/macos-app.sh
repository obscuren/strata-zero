#!/bin/bash
# Builds target/smoke/macos/Strata Zero.app: the game in its own window, for Apple
# Silicon and Intel in one binary, ad-hoc signed (enough to run on this Mac; a release
# needs a Developer ID signature and notarization).
#
#   scripts/macos-app.sh            build the app
#   scripts/macos-app.sh --icon     also regenerate crates/strata-zero/macos/AppIcon.icns
set -euo pipefail

cd "$(dirname "$0")/.."
target=target/smoke
app="$target/macos/Strata Zero.app"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)

if [ "${1:-}" = "--icon" ]; then
    shots=$(mktemp -d)
    SHOT_DIR="$shots" cargo test -q -p strata-zero --features window --target-dir "$target" \
        window::tests::app_icon -- --ignored
    set=$(mktemp -d)/AppIcon.iconset
    mkdir -p "$set"
    for s in 16 32 128 256 512; do
        sips -z $s $s "$shots/icon.png" --out "$set/icon_${s}x${s}.png" >/dev/null
        sips -z $((s * 2)) $((s * 2)) "$shots/icon.png" --out "$set/icon_${s}x${s}@2x.png" >/dev/null
    done
    iconutil -c icns "$set" -o crates/strata-zero/macos/AppIcon.icns
fi

bins=()
for triple in aarch64-apple-darwin x86_64-apple-darwin; do
    if rustup target list --installed | grep -qx "$triple"; then
        cargo build --release -p strata-zero --features window --target "$triple" \
            --target-dir "$target"
        bins+=("$target/$triple/release/strata")
    fi
done

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
lipo -create "${bins[@]}" -output "$app/Contents/MacOS/strata"
sed "s/@VERSION@/$version/g" crates/strata-zero/macos/Info.plist > "$app/Contents/Info.plist"
if [ -f crates/strata-zero/macos/AppIcon.icns ]; then
    cp crates/strata-zero/macos/AppIcon.icns "$app/Contents/Resources/"
fi
codesign --force --sign - "$app"
echo "$app"
