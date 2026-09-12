# Project rules

- Read docs/architecture.md, docs/requirements.md, docs/development.md, architecture/spec.toml and the current ADR before changing behavior.
- Architecture first: if a requirement cannot fit existing boundaries, update requirements, ADR, architecture and the versioned harness contract with negative tests BEFORE feature implementation. No bypass or temporary convenience exception.
- Keep dependencies within architecture/spec.toml: core is pure; features defines category operations; runtime owns use cases/ports; adapters owns device/audit I/O; mcp owns protocol; server only composes. Core has no OS, async runtime, network, logging, or MCP dependencies.
- Key custody and crypto are separate: key-sources owns file/env/container access, crypto-age owns age over supplied streams, runtime::protection owns ports and purpose-specific material. Neither infrastructure crate depends on the other; MCP cannot import protection types. No real Vault/key access for development fixtures.
- Every callable device operation passes through the runtime dispatcher, authorization, and audit sink. MCP handlers must never spawn processes or access device configuration directly.
- Permissions come from operator-owned configuration and server-owned operation metadata, never from client arguments or MCP annotations.
- Unknown categories and operations are denied. Access and execution are independent. Extensions do not override built-ins.
- Never log raw arguments, device responses, credentials, private keys, configuration contents, or raw backend errors. Use synthetic fixtures only.
- Device outputs are untrusted data. Default result projection exposes only explicitly selected fields; no wildcard output for sensitive configuration.
- Repository work does not authorize changes to a live router or public pushes. Use fake backends for tests.
- Keep unimplemented coverage and unmeasured performance explicit. Never describe fixture tests as device validation.
- Run tools/Test-Repository.ps1 before committing, and resolve all failures. No bypasses.
- Change files with apply_patch. Do not weaken boundaries to make a feature easier to implement.
- Windows/Linux/macOS are required host targets across all shared features. Never confuse host-native paths/commands with router POSIX actions, use remote-to-local fallback, treat Unix modes as macOS ACL proof, or report WSL as native Windows validation. Platform protection and persistent remote execution have separate infrastructure owners under ADR 0004.
