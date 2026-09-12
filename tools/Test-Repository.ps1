[CmdletBinding()]
param([string]$BaseRef = 'HEAD')

$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $repositoryRoot
try {
    & git diff --check
    if ($LASTEXITCODE -ne 0) { throw 'Working tree whitespace validation failed.' }
    & git diff --cached --check
    if ($LASTEXITCODE -ne 0) { throw 'Staged whitespace validation failed.' }
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        & cargo run --locked -p xtask -- architecture --base $BaseRef
        if ($LASTEXITCODE -ne 0) { throw 'Architecture contract validation failed.' }
        & cargo test --locked -p xtask
        if ($LASTEXITCODE -ne 0) { throw 'Architecture negative regression tests failed.' }
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
        & cargo clippy --workspace --all-targets --locked -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Rust lint checks failed.' }
        & cargo test --workspace --locked
        if ($LASTEXITCODE -ne 0) { throw 'Repository tests failed.' }
        & cargo build --workspace --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    } elseif (Get-Command wsl -ErrorAction SilentlyContinue) {
        # Use the existing Ubuntu/Nix development environment without editing its profile.
        & wsl -d Ubuntu --cd $repositoryRoot -- /nix/var/nix/profiles/default/bin/nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#rustfmt nixpkgs#clippy nixpkgs#gcc --command env CARGO_TARGET_DIR=/tmp/openwrt-mcp-cargo-target sh tools/test.sh $BaseRef
        if ($LASTEXITCODE -ne 0) { throw 'WSL repository checks failed.' }
    } else {
        throw 'Install a Rust toolchain and C linker, then run this script again.'
    }
} finally {
    Pop-Location
}
