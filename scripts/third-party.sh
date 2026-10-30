#!/bin/bash
# Writes THIRD-PARTY-NOTICES.txt: the licence notices of every crate Strata Zero is built
# from (the terminal game and the window, for every release target) and of Fira Code,
# which the window build carries. Every release artifact ships it beside LICENSE.
#
#   scripts/third-party.sh           regenerate it, after a dependency changes
#   scripts/third-party.sh --check   fail if it is out of date, or if a crate's licence is
#                                    not one about.toml accepts (CI)
#
# Needs cargo-about at the version below:
#   cargo install cargo-about --locked --version 0.9.2 --features cli
set -euo pipefail

cd "$(dirname "$0")/.."
version=0.9.2
out=THIRD-PARTY-NOTICES.txt

check=
for arg in "$@"; do
    case "$arg" in
        --check) check=1 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

if [ "$(cargo about --version 2>/dev/null)" != "cargo-about $version" ]; then
    echo "error: needs cargo-about $version:" \
        "cargo install cargo-about --locked --version $version --features cli" >&2
    exit 1
fi

new=$(mktemp)
trap 'rm -f "$new"' EXIT
# Every feature, so the window's and the desktop apps' crates are in; about.toml names
# the release targets. Some crates' licence files have CRLF line ends or trail blank
# lines: the file is plain LF text with one blank line at most.
{
    cargo about generate --workspace --all-features --locked --fail scripts/third-party.hbs
    cat <<'EOF'

--------------------------------------------------------------------------------
SIL Open Font License 1.1, for:
  Fira Code 6.2 (https://github.com/tonsky/FiraCode)

EOF
    cat crates/strata-zero/fonts/OFL.txt
} | tr -d '\r' | cat -s >"$new"

if [ -n "$check" ]; then
    if ! cmp -s "$out" "$new"; then
        echo "error: $out is out of date: run scripts/third-party.sh and commit it" >&2
        diff -u "$out" "$new" | head -60 >&2 || true
        exit 1
    fi
    echo "$out is up to date"
else
    cat "$new" >"$out"
    echo "$out"
fi
