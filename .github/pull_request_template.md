## Requirement and acceptance

Describe the requirement, affected categories/permissions, and tests proving the result.

## Architecture fit

- [ ] This fits the current ownership and dependency contract; identify the owning modules.
- [ ] If boundaries changed, requirements, a new ADR, the versioned contract and harness rejection tests were updated first, followed by a validated architecture checkpoint before functional work. Link the checkpoint.
- [ ] No alternate device path, blanket exception, ignored failure or permission bypass was added.

## Verification and scope

- [ ] The complete repository gate passes; record the baseline used.
- [ ] Denial, invalid inputs, output privacy, failure and resource-bound tests cover the change.
- [ ] Coverage distinguishes synthetic fixtures from emulator/device acceptance.
- [ ] No secrets, raw configuration or decrypted backups are included.
- [ ] Any router access/publication had separate authorization; otherwise none occurred.

State remaining limitations and measured performance impact. Static checks supplement, not replace, semantic/security review.
