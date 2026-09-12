# Project rules

- Read docs/architecture.md, docs/requirements.md, docs/development.md, architecture/spec.toml and the current ADR before changing behavior.
- Architecture first: if a requirement cannot fit existing boundaries, update requirements, ADR, architecture and the versioned harness contract with negative tests BEFORE feature implementation. No bypass or temporary convenience exception.
- Keep dependencies within architecture/spec.toml: core is pure; features defines category operations; runtime owns use cases/ports; adapters owns device/audit I/O; mcp owns protocol; server only composes. Core has no OS, async runtime, network, logging, or MCP dependencies.
- Every callable device operation passes through the runtime dispatcher, authorization, and audit sink. MCP handlers must never spawn processes or access device configuration directly.
- Permissions come from operator-owned configuration and server-owned operation metadata, never from client arguments or MCP annotations.
- Unknown categories and operations are denied. Access and execution are independent. Extensions do not override built-ins.
- Never log raw arguments, device responses, credentials, private keys, configuration contents, or raw backend errors. Use synthetic fixtures only.
- Device outputs are untrusted data. Default result projection exposes only explicitly selected fields; no wildcard output for sensitive configuration.
- Repository work does not authorize changes to a live router or public pushes. Use fake backends for tests.
- Keep unimplemented coverage and unmeasured performance explicit. Never describe fixture tests as device validation.
- Run tools/Test-Repository.ps1 before committing, and resolve all failures. No bypasses.
- Change files with apply_patch. Do not weaken boundaries to make a feature easier to implement.
