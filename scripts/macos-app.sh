#!/bin/bash
# Builds target/smoke/macos/Strata Zero.app: the game in its own window, for Apple
# Silicon and Intel in one binary (whichever of the two targets are installed).
#
#   scripts/macos-app.sh            build the app
#   scripts/macos-app.sh --dmg      also wrap it in target/smoke/macos/strata-zero-mac-app.dmg
#   scripts/macos-app.sh --icon     also regenerate crates/strata-zero/macos/AppIcon.icns
#
# Signing, from the environment:
#   STRATA_SIGN_IDENTITY   a codesign identity ("Developer ID Application: … (TEAM)");
#                          without it the app is signed ad hoc, enough for this Mac only
#   APPLE_API_KEY          an App Store Connect API key (.p8 text, or base64 of it), with
#   APPLE_API_KEY_ID       its key id and
#   APPLE_API_ISSUER       its issuer id: with all three and an identity, the DMG is
#                          notarized and the ticket stapled to it
#   STRATA_REQUIRE_SIGNING set to 1 to fail rather than build an unsigned app
set -euo pipefail

cd "$(dirname "$0")/.."
target=target/smoke
out="$target/macos"
app="$out/Strata Zero.app"
dmg="$out/strata-zero-mac-app.dmg"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
identity="${STRATA_SIGN_IDENTITY:--}"

want_dmg=
want_icon=
for arg in "$@"; do
    case "$arg" in
        --dmg) want_dmg=1 ;;
        --icon) want_icon=1 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

if [ "$identity" = "-" ] && [ "${STRATA_REQUIRE_SIGNING:-}" = "1" ]; then
    echo "error: STRATA_REQUIRE_SIGNING is set but no STRATA_SIGN_IDENTITY was given;" >&2
    echo "the Developer ID secrets are missing (see README, Release)." >&2
    exit 1
fi

if [ -n "$want_icon" ]; then
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

if [ "$identity" = "-" ]; then
    codesign --force --sign - "$app"
else
    # Notarization wants the hardened runtime and a secure timestamp.
    codesign --force --options runtime --timestamp --sign "$identity" "$app"
fi
codesign --verify --strict "$app"
echo "$app"

if [ -z "$want_dmg" ]; then
    exit 0
fi

# The DMG holds the app and a link to /Applications to drag it onto.
stage=$(mktemp -d)
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -quiet -volname "Strata Zero" -srcfolder "$stage" -ov -format UDZO "$dmg"
rm -rf "$stage"

if [ "$identity" != "-" ]; then
    codesign --force --timestamp --sign "$identity" "$dmg"
    if [ -n "${APPLE_API_KEY:-}" ] && [ -n "${APPLE_API_KEY_ID:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ]; then
        keydir=$(mktemp -d)
        key="$keydir/AuthKey_${APPLE_API_KEY_ID}.p8"
        if printf '%s' "$APPLE_API_KEY" | grep -q "BEGIN PRIVATE KEY"; then
            printf '%s\n' "$APPLE_API_KEY" > "$key"
        else
            printf '%s' "$APPLE_API_KEY" | base64 --decode > "$key"
        fi
        result="$keydir/notary.json"
        xcrun notarytool submit "$dmg" --key "$key" --key-id "$APPLE_API_KEY_ID" \
            --issuer "$APPLE_API_ISSUER" --wait --output-format json > "$result"
        status=$(plutil -extract status raw -o - "$result")
        if [ "$status" != "Accepted" ]; then
            id=$(plutil -extract id raw -o - "$result")
            echo "error: notarization came back $status" >&2
            xcrun notarytool log "$id" --key "$key" --key-id "$APPLE_API_KEY_ID" \
                --issuer "$APPLE_API_ISSUER" >&2 || true
            rm -rf "$keydir"
            exit 1
        fi
        rm -rf "$keydir"
        xcrun stapler staple "$dmg"
    elif [ "${STRATA_REQUIRE_SIGNING:-}" = "1" ]; then
        echo "error: signed, but the App Store Connect key for notarizing is missing" >&2
        exit 1
    fi
fi
echo "$dmg"
