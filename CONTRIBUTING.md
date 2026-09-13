# Contributing

Read [AGENTS.md](AGENTS.md), [requirements](docs/requirements.md), [architecture](docs/architecture.md), [development](docs/development.md), and the [active contract](architecture/spec.toml) first.

State the requirement, affected categories, permissions and acceptance tests. If it fits the existing architecture, implement within the owning crate. If it does not, stop functional work and first extend the architecture, ADR, contract and rejection tests. Keep a validated architecture checkpoint before the incompatible feature. Do not add bypasses, ignore rules or direct I/O to an inappropriate layer.

Use synthetic fixtures. Repository work does not authorize live router changes, credentials, decrypted backups or publication. Claims must distinguish source review, host fixtures, emulator testing and real device acceptance. A generic command action is not proof of complete feature support.

Run `tools/Test-Repository.ps1` or `sh tools/test.sh` before committing. The gate checks architecture/evolution, negative cases, formatting, strict linting, behavior and a release build. CI uses the same gate. Include coverage and performance impacts where relevant, and do not describe a host binary as an OpenWrt release artifact.

Project-owned code and contributions are licensed under the [MIT License](LICENSE). Submit only material you have the right to contribute under these terms. Preserve existing copyright and license notices; third-party dependencies retain their own licenses and must be reviewed before redistribution.
