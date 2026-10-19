# Installs Strata Zero on Windows: runs dist's PowerShell installer from the
# latest release. Puts `strata` in ~/.cargo/bin.
#
#   irm https://github.com/obscuren/strata-zero/raw/main/install.ps1 | iex
$ErrorActionPreference = 'Stop'
Invoke-RestMethod https://github.com/obscuren/strata-zero/releases/latest/download/strata-zero-installer.ps1 | Invoke-Expression
