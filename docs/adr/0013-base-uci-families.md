# ADR 0013: Broader closed base-service UCI observations

Status: accepted architecture declaration. Validate and commit this architecture-only checkpoint before production enum/catalog migration.

## Decision and scope

The existing six UCI profiles exclude important base-service configuration families. V12 can already represent their selected scalar/string-list options faithfully, but its exact recipe allowlist cannot admit new section types. Expand that finite list explicitly, not through arbitrary UCI, caller-selected types or a raw configuration export.

Keep the original six profiles and add exactly eighteen:

| Category Read | Fixed config | Additional exact section types |
| --- | --- | --- |
| system | system | timeserver |
| network | network | device, bridge-vlan, route, route6, rule, rule6 |
| firewall | firewall | zone, forwarding, rule, redirect, nat |
| dhcp_dns | dhcp | dhcp, host, domain, cname |
| storage | fstab | global, swap |

Core::uci owns the non-serializable closed enum and exact config/type/category mapping. Existing validation still requires parameterless Ubus uci/get with exactly config/type, the mapped Read grant, a bounded /values section map, root /error absence and an exact required section-type enum. All custom UCI definitions remain rejected even with Extensions.Write/Execute. No new capability object, probe, registry schema, version-only compatibility proof, action variant or backend route.

The same twelve crates retain their directories, imports and dependency contracts. Features category modules assemble reviewed declarations through the shared pure definition helper; private category submodules may group configuration declarations as they grow. Core does not own an MCP tool-name catalog. Runtime remains the only policy/audit/dispatch use case, codec strictly decodes supplied JSON, and MCP receives bounded normalized results only. No OS-specific feature behavior or host-to-local fallback is introduced.

## Output, disclosure and limitations

Only the independently documented fields in [base-uci-observations.md](../base-uci-observations.md) are initial consumers. New tools start with response v1 even if they contain TextOption; the two existing v2 contracts are unchanged. Scalar options preserve raw text, and TextOption preserves string/list representation without interpreting defaults, negation, wildcard ports, names or addresses. No option invokes its downstream service or parser. For example, rule action/target text is observation, not an executable command.

All existing bounds remain: 128 section rows, 128 values per option, 256 shared scanned/emitted rows/values, 8 collection nodes, 64 fields per record, 64-KiB normalized and 256-KiB MCP output. Split future reviewed views if a feature exceeds finite schema capacity; do not increase limits or return arbitrary configuration to force coverage. Whole-response rejection and non-atomic shared pending-delta semantics remain. Input-signature compatibility does not prove a config file exists or a service uses the observed sections.

Reads deliberately disclose selected topology, firewall policy, client reservations, addresses, domain names and filesystem identifiers under the mapped category. Missing options stay missing and invalid semantic strings are not silently fixed. Exclude credentials, free-form extra configuration, executable scripts, mount options, wireless BSS/SSID/keys, VPN material and undeclared options. Device-controlled text can contain arbitrary data even in a selected field; this is not a secret-content classifier.

These observations are **not** a complete topology/effect graph. Dynamic netifd/procd data, nftables runtime rules, include scripts, addon services and indirect reload effects remain outside them. No mutation may use these partial reads alone to certify lan3/IPTV safety. No service lifecycle, mount/swap, package, firmware, encrypted backup or rollback workflow is admitted here.

## Harness and acceptance

Version 13 requires the exact twenty-four profile set. Versions 11/12 retain exactly the earlier six; downgrade tests construct the appropriate older contract before testing prohibited re-admission. Missing/duplicate/extra/misspelled/dynamic profiles, category/invocation weakening, unreviewed probes, removed required suites and changed budgets remain failures. Add direct and assembled negative regressions before behavior. No test scaffold is feature evidence.

After the checkpoint, tests must independently bind every enum recipe/category, reject cross-product config/type substitutions and all custom variants, validate exact declared fields and omitted private fields, and exercise empty/absent/invalid/oversized late data. Every new MCP operation requires actual dispatcher/protocol tests for fixed arguments, discovery/direct-call category isolation, no execute requirement, malformed response errors and payload-free audit. Emulator evidence states exact populated versus empty/error scope and never implies physical-target acceptance. A matching source recipe does not prove full management coverage.
