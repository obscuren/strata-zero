#!/bin/bash
# Puts the desktop apps at the top of a GitHub Release's notes: the Mac, Windows and
# Linux apps, in that order. dist writes the notes, and its download table lists only
# the files dist builds, so the apps that .github/workflows/build-*-app.yml add would
# otherwise go unmentioned.
#
#   scripts/app-notes.sh v0.1.0
#
# gh must be signed in with write access to the repository; in CI, GH_TOKEN carries it.
# GH_REPO names the repository (default obscuren/strata-zero). An app the release
# doesn't have, or that the notes already link, is left out, so it can run again.
# MAC_APP_UNSIGNED=1, as for the release, describes a Mac app that isn't notarized.
set -euo pipefail

cd "$(dirname "$0")/.."
tag="${1:?usage: scripts/app-notes.sh <tag>}"
repo="${GH_REPO:-obscuren/strata-zero}"
base="https://github.com/$repo/releases/download/$tag"

notes=$(gh release view "$tag" --repo "$repo" --json body --jq .body)
assets=$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')

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

sections=()
# Adds a section for `file` under `heading`, when the release has it and the notes don't
# link it yet.
section() {
    local file=$1 heading=$2 text=$3
    if ! grep -qxF "$file" <<< "$assets" || grep -qF "$base/$file" <<< "$notes"; then
        return 0
    fi
    sections+=("## $heading"$'\n\n'"[$file]($base/$file): $text")
}
section strata-zero-mac-app.dmg "Mac app" \
    "Strata Zero in a window of its own, one app for $macs. Open the DMG and drag Strata Zero into Applications. $trust"
section strata-zero-windows-app.zip "Windows app" \
    "Strata Zero in a window of its own, for 64-bit Windows 10 and 11. Unzip it and run \`Strata Zero.exe\`; your saves are kept in \`%APPDATA%\\Strata Zero\`. It isn't signed yet, so Windows SmartScreen asks the first time: choose More info, then Run anyway."
section strata-zero-linux-app.tar.gz "Linux app" \
    "Strata Zero in a window of its own, for 64-bit Linux on X11 or Wayland, drawn through Vulkan where there is a driver. Unpack it and run \`strata-zero\`, or run \`./install.sh\` once to add Strata Zero to your app menu; your saves are kept in \`~/.local/share/strata-zero\`."

if [ ${#sections[@]} -eq 0 ]; then
    echo "$tag: the notes already link every app the release has"
    exit 0
fi
file=$(mktemp)
trap 'rm -f "$file"' EXIT
{
    printf '%s\n\n' "${sections[@]}"
    printf '%s\n' "$notes"
} > "$file"
gh release edit "$tag" --repo "$repo" --notes-file "$file" > /dev/null
echo "$tag: the notes now link ${#sections[@]} more app(s)"
