#!/bin/bash
# Builds target/smoke/linux/strata-zero-linux-app.tar.gz: the game in its own window for
# Linux (the `app` feature), as `strata-zero/strata-zero` beside its desktop file, icon,
# install.sh, the game's license, the font's and the third-party notices. Runs on Linux.
#
#   scripts/linux-app.sh          build the archive
#   scripts/linux-app.sh --icon   only regenerate crates/strata-zero/linux/strata-zero.png
#                                 from the app_icon test (on any machine)
set -euo pipefail

cd "$(dirname "$0")/.."
target=target/smoke
out="$target/linux"
dir="$out/strata-zero"
archive="$out/strata-zero-linux-app.tar.gz"

for arg in "$@"; do
    case "$arg" in
        --icon)
            shots=$(mktemp -d)
            SHOT_DIR="$shots" cargo test -q -p strata-zero --features window \
                --target-dir "$target" window::tests::app_icon -- --ignored
            cp "$shots/strata-zero.png" crates/strata-zero/linux/strata-zero.png
            echo crates/strata-zero/linux/strata-zero.png
            exit 0
            ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

cargo build --release -p strata-zero --features app --target-dir "$target"
rm -rf "$dir" "$archive"
mkdir -p "$dir"
install -m 755 "$target/release/strata" "$dir/strata-zero"
install -m 755 crates/strata-zero/linux/install.sh "$dir/install.sh"
install -m 644 crates/strata-zero/linux/strata-zero.desktop crates/strata-zero/linux/strata-zero.png \
    LICENSE crates/strata-zero/fonts/OFL.txt THIRD-PARTY-NOTICES.txt "$dir/"
tar -C "$out" -czf "$archive" strata-zero
echo "$archive"
