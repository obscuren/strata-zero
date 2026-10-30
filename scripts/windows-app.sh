#!/bin/bash
# Builds target/smoke/windows/strata-zero-windows-app.zip: the game in its own window for
# Windows (the `app` feature), as `Strata Zero/Strata Zero.exe` beside the game's license,
# the font's and the third-party notices. Runs on Windows under Git Bash, as CI's
# `shell: bash` does.
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

# The C runtime linked in, as dist builds the terminal game, so the exe starts on a PC
# without the Visual C++ redistributable.
RUSTFLAGS="${RUSTFLAGS:-} -Ctarget-feature=+crt-static" \
    cargo build --release -p strata-zero --features app --target-dir "$target"
exe="$target/release/strata.exe"
if grep -qia 'vcruntime140' "$exe"; then
    echo "error: $exe still needs VCRUNTIME140.dll" >&2
    exit 1
fi
rm -rf "$dir" "$zip"
mkdir -p "$dir"
cp "$exe" "$dir/Strata Zero.exe"
cp LICENSE "$dir/LICENSE.txt"
cp crates/strata-zero/fonts/OFL.txt "$dir/OFL.txt"
cp THIRD-PARTY-NOTICES.txt "$dir/THIRD-PARTY-NOTICES.txt"
# Windows' own tar (bsdtar, since Windows 10) writes the zip: Git Bash's tar can't, and
# Windows PowerShell's Compress-Archive writes `\` into the paths, which unzip elsewhere
# takes as part of the name.
"$(cygpath -u "$SYSTEMROOT")/System32/tar.exe" -a -c -f "$(cygpath -w "$zip")" \
    -C "$(cygpath -w "$out")" "Strata Zero"
echo "$zip"
