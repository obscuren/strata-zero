#!/bin/bash
# Packages target/smoke/macos/Strata Zero.app, as scripts/macos-app.sh built it, into
# target/smoke/macos/strata-zero-mac-app.dmg: signs the app, wraps it in a DMG with a
# link to /Applications, signs the DMG, notarizes it and staples the ticket. It runs no
# cargo, so the signing secrets it reads never reach a dependency's build script.
#
# From the environment:
#   STRATA_SIGN_IDENTITY   a codesign identity ("Developer ID Application: … (TEAM)");
#                          without it the app stays signed ad hoc and nothing is notarized
#   APPLE_API_KEY          an App Store Connect API key (.p8 text, or base64 of it), with
#   APPLE_API_KEY_ID       its key id and
#   APPLE_API_ISSUER       its issuer id: with all three and an identity, the DMG is
#                          notarized and stapled
#   STRATA_REQUIRE_SIGNING set to 1 to fail rather than package an unsigned or
#                          un-notarized app
set -euo pipefail

cd "$(dirname "$0")/.."
out=target/smoke/macos
app="$out/Strata Zero.app"
dmg="$out/strata-zero-mac-app.dmg"
identity="${STRATA_SIGN_IDENTITY:-}"
require="${STRATA_REQUIRE_SIGNING:-}"

if [ ! -d "$app" ]; then
    echo "error: $app is missing; run scripts/macos-app.sh first" >&2
    exit 1
fi
if [ -z "$identity" ] && [ "$require" = "1" ]; then
    echo "error: STRATA_REQUIRE_SIGNING is set but there is no STRATA_SIGN_IDENTITY;" >&2
    echo "the Developer ID secrets are missing (README, \"Signing the Mac app\")." >&2
    exit 1
fi

# Runs "$@" up to `tries` times, `pause` seconds apart: hdiutil and stapler fail now and
# then on a busy machine (hdiutil's "Resource busy"; a ticket not out yet).
retry() {
    local tries=$1 pause=$2
    shift 2
    local n=1
    until "$@"; do
        if [ "$n" -ge "$tries" ]; then
            return 1
        fi
        echo "retrying ($n/$tries failed): $*" >&2
        sleep "$pause"
        n=$((n + 1))
    done
}

if [ -n "$identity" ]; then
    # Notarization wants the hardened runtime and a secure timestamp.
    codesign --force --options runtime --timestamp --sign "$identity" "$app"
fi
codesign --verify --strict "$app"

stage=$(mktemp -d)
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
retry 3 10 hdiutil create -quiet -volname "Strata Zero" -srcfolder "$stage" -ov \
    -format UDZO "$dmg"
rm -rf "$stage"

if [ -z "$identity" ]; then
    echo "$dmg (unsigned)"
    exit 0
fi
codesign --force --timestamp --sign "$identity" "$dmg"

if [ -z "${APPLE_API_KEY:-}" ] || [ -z "${APPLE_API_KEY_ID:-}" ] || [ -z "${APPLE_API_ISSUER:-}" ]; then
    if [ "$require" = "1" ]; then
        echo "error: signed, but the App Store Connect key for notarizing is missing" >&2
        exit 1
    fi
    echo "$dmg (signed, not notarized)"
    exit 0
fi

keydir=$(mktemp -d)
trap 'rm -rf "$keydir"' EXIT
key="$keydir/AuthKey_${APPLE_API_KEY_ID}.p8"
if printf '%s' "$APPLE_API_KEY" | grep -q "BEGIN PRIVATE KEY"; then
    printf '%s\n' "$APPLE_API_KEY" > "$key"
else
    printf '%s' "$APPLE_API_KEY" | base64 --decode > "$key"
fi
auth=(--key "$key" --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER")

# notarytool's exit code doesn't say why it failed; read its answer either way.
result="$keydir/notary.json"
xcrun notarytool submit "$dmg" "${auth[@]}" --wait --timeout 30m --output-format json \
    > "$result" || true
cat "$result"
echo
status=$(plutil -extract status raw -o - "$result" 2>/dev/null || echo "no answer")
if [ "$status" != "Accepted" ]; then
    echo "error: notarization came back: $status" >&2
    if id=$(plutil -extract id raw -o - "$result" 2>/dev/null); then
        xcrun notarytool log "$id" "${auth[@]}" >&2 || true
    fi
    exit 1
fi
retry 5 15 xcrun stapler staple "$dmg"
echo "$dmg (signed, notarized)"
