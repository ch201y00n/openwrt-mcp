[CmdletBinding()]
param(
    [string]$BaseRef = 'HEAD',
    [switch]$UseWsl
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($BaseRef) -or $BaseRef -eq '0000000000000000000000000000000000000000') {
    $BaseRef = 'HEAD'
}
Push-Location -LiteralPath $repositoryRoot
try {
    & git diff --check
    if ($LASTEXITCODE -ne 0) { throw 'Working tree whitespace validation failed.' }
    & git diff --cached --check
    if ($LASTEXITCODE -ne 0) { throw 'Staged whitespace validation failed.' }
    if ($UseWsl) {
        if (-not (Get-Command wsl -ErrorAction SilentlyContinue)) {
            throw 'Explicit WSL validation requested, but WSL is unavailable.'
        }
        Write-Host 'Validation environment: Linux on WSL (not native Windows acceptance).'
        # This explicit developer convenience uses the existing Ubuntu/Nix environment.
        # Native CI never calls this branch; no profile is installed or changed.
        & wsl -d Ubuntu --cd $repositoryRoot -- /nix/var/nix/profiles/default/bin/nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#rustfmt nixpkgs#clippy nixpkgs#gcc --command env CARGO_TARGET_DIR=/tmp/openwrt-mcp-cargo-target sh tools/test.sh $BaseRef
        if ($LASTEXITCODE -ne 0) { throw 'Linux-on-WSL repository checks failed.' }
    } elseif (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host ('Validation environment: native {0} ({1}).' -f [System.Runtime.InteropServices.RuntimeInformation]::OSDescription, [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)
        & rustc --version --verbose
        if ($LASTEXITCODE -ne 0) { throw 'Cannot identify the native Rust toolchain.' }
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
    } else {
        throw 'Install a native Rust toolchain and C linker. For explicitly Linux-only development validation, use -UseWsl.'
    }
} finally {
    Pop-Location
}
