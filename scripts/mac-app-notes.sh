#!/bin/bash
# Puts the Mac app at the top of a GitHub Release's notes. dist writes the notes, and its
# download table lists only the files dist builds, so the DMG that
# .github/workflows/build-mac-app.yml adds would otherwise go unmentioned.
#
#   scripts/mac-app-notes.sh v0.1.0
#
# gh must be signed in with write access to the repository; in CI, GH_TOKEN carries it.
# GH_REPO names the repository (default obscuren/strata-zero). When the notes already
# link the DMG it changes nothing, so it can run again. MAC_APP_UNSIGNED=1, as for the
# release, describes a DMG that isn't notarized.
set -euo pipefail

cd "$(dirname "$0")/.."
tag="${1:?usage: scripts/mac-app-notes.sh <tag>}"
repo="${GH_REPO:-obscuren/strata-zero}"
dmg=strata-zero-mac-app.dmg
url="https://github.com/$repo/releases/download/$tag/$dmg"

notes=$(gh release view "$tag" --repo "$repo" --json body --jq .body)
if grep -qF "$url" <<< "$notes"; then
    echo "$tag: the notes already link $dmg"
    exit 0
fi
assets=$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')
if ! grep -qxF "$dmg" <<< "$assets"; then
    echo "error: release $tag has no $dmg" >&2
    exit 1
fi

# The oldest macOS the app opens on, from its Info.plist.
min=$(sed -n '/LSMinimumSystemVersion/{n;s/.*<string>\(.*\)<\/string>.*/\1/p;}' \
    crates/strata-zero/macos/Info.plist)
macs="Apple Silicon and Intel Macs"
if [ -n "$min" ]; then
    macs="$macs, macOS $min or later"
fi
if [ "${MAC_APP_UNSIGNED:-}" = "1" ]; then
    trust="This build isn't notarized: macOS blocks it until you allow it under System Settings › Privacy & Security."
else
    trust="It's signed with a Developer ID and notarized by Apple."
fi

file=$(mktemp)
trap 'rm -f "$file"' EXIT
cat > "$file" <<EOF
## Mac app

[$dmg]($url): Strata Zero in a window of its own, one app for $macs. Open the DMG and drag Strata Zero into Applications. $trust

$notes
EOF
gh release edit "$tag" --repo "$repo" --notes-file "$file" > /dev/null
echo "$tag: the notes now link $dmg"
