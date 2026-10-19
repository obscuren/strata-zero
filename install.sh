#!/bin/sh
# Installs Strata Zero: fetches dist's installer from the latest release and
# runs it, passing on any arguments. Puts `strata` in ~/.cargo/bin.
#
#   curl -fsSL https://github.com/obscuren/strata-zero/raw/main/install.sh | sh
#
# The installer is downloaded first and run second, so a failed download stops
# here instead of handing half a script to sh; everything sits in main, so a
# truncated copy of this file runs nothing.
set -eu

main() {
    url=https://github.com/obscuren/strata-zero/releases/latest/download/strata-zero-installer.sh
    tmp=$(mktemp)
    trap 'rm -f "$tmp"' EXIT
    curl --proto '=https' --tlsv1.2 -LsSf -o "$tmp" "$url"
    sh "$tmp" "$@"
}

main "$@"
