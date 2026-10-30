#!/bin/bash
# Builds target/smoke/windows/strata-zero-windows-app.zip: the game in its own window for
# Windows (the `app` feature), as `Strata Zero/Strata Zero.exe` beside the game's license
# and the font's. Runs on Windows under Git Bash, as CI's `shell: bash` does.
#
#   scripts/windows-app.sh          build the zip
#   scripts/windows-app.sh --icon   only regenerate crates/strata-zero/windows/AppIcon.ico
#                                   from the app_icon test (on any machine)
set -euo pipefail

cd "$(dirname "$0")/.."
target=target/smoke
out="$target/windows"
dir="$out/Strata Zero"
zip="$out/strata-zero-windows-app.zip"

for arg in "$@"; do
    case "$arg" in
        --icon)
            shots=$(mktemp -d)
            SHOT_DIR="$shots" cargo test -q -p strata-zero --features window \
                --target-dir "$target" window::tests::app_icon -- --ignored
            cp "$shots/icon.ico" crates/strata-zero/windows/AppIcon.ico
            echo crates/strata-zero/windows/AppIcon.ico
            exit 0
            ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

cargo build --release -p strata-zero --features app --target-dir "$target"
rm -rf "$dir" "$zip"
mkdir -p "$dir"
cp "$target/release/strata.exe" "$dir/Strata Zero.exe"
cp LICENSE "$dir/LICENSE.txt"
cp crates/strata-zero/fonts/OFL.txt "$dir/OFL.txt"
powershell -NoProfile -Command \
    "Compress-Archive -Path '$(cygpath -w "$dir")' -DestinationPath '$(cygpath -w "$zip")'"
echo "$zip"
