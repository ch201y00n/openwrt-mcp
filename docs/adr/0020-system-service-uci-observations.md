# ADR 0020: Closed system-service configuration observations

Status: accepted for an architecture-only checkpoint before implementation.
Scope: selected configuration, not service control, credentials or effective state.

## Ownership and recipes

LEDs, Dropbear, uHTTPd and odhcpd are additional base-system management surfaces.
The current twenty-four-recipe UCI allowlist blocks them, so extend that exact set
under a reviewed checkpoint. Reuse all twelve owners, core UciReadProfile validation,
category-owned feature definitions, typed projection, same-target capabilities,
dispatcher authorization/audit and local/SSH codec. No new dependency, probe, port,
output form, OS condition, generic path/action or resource limit.

| Config / section type | Requirement | Planned operation |
| --- | --- | --- |
| system / led | system.read | system_led_configuration |
| dropbear / dropbear | system.read | system_dropbear_configuration |
| uhttpd / uhttpd | system.read | system_uhttpd_configuration |
| dhcp / odhcpd | dhcp_dns.read | dhcp_odhcpd_configuration |

SSH/web administration configuration belongs to System, not generic Services
runtime metadata. An operator may deny individual operations to withhold policy
flags, listen endpoints or interface names. Authentication flags are not passwords.
Generic custom UCI remains rejected even with privileged extension grants; the
client cannot select config/type/category or install a recipe.

## Output and limits

Each new operation is parameterless, sends only its fixed `uci.get` config/type
pair, and requires the existing reviewed input signature on the same target. Use
the existing section map with bounded section identity, exact required type,
anonymous boolean, index and reviewed optional fields. Scalar options remain text;
selected text/list fields retain kind/values, without splitting, coercion,
deduplication or synthesized defaults. New response contracts begin at v1.

LED fields may describe name/sysfs/trigger/device, default/inversion, brightness,
timing and bounded mode/port choices. Exclude arbitrary message text, scripts and
device/sysfs contents. Dropbear may expose enable/authentication/forwarding flags,
configured port/interfaces, keepalive/idle/auth-attempt/window limits and mDNS,
but no forced command, key/key path or banner data. uHTTPd may expose listen
endpoints, redirect, request/connection limits, timeouts, keepalive and request
security flags, but no key/cert/auth paths, document-root content or executable
CGI/Lua/ucode/interpreter/ubus handler configuration. odhcpd may expose main-DHCP
and log flags plus lease/hosts/PIO paths, but no contents, client identities or
lease-trigger commands. Observed paths are never opened, expanded or executed.
Before advertising support, document exact field names/bounds and verify them
with independent fixtures, not expectations derived from production definitions.

Retain every existing row/item/string/schema/normalized/MCP budget, all previous
response IDs and snapshot lifecycles. Present malformed fields or root errors fail;
missing optional fields stay omitted. No raw subtree, key/certificate/authorized-key
content, arbitrary command, write, init script, sysfs read or network request.
Audit never contains arguments, results or configuration values.

## Version and source semantics

This remains rpcd's sessionless shared-delta, non-atomic view, potentially including
pending changes. It is not committed-only configuration, installed-package proof,
daemon status, effective authentication policy, live listening endpoints, LED state
or current leases. Empty sections do not prove absence/inactivity. Section names
and indices are not durable mutation handles. Versions/variants may omit options;
do not fill defaults or infer support from version strings. Capability, permission,
projected configuration and actual device acceptance remain separate facts.

## Checkpoint and validation

Extend only the versioned uci_read_contract recipe set and negative harness.
v11/v12 retain six, v13..v19 retain twenty-four, and v20 requires all twenty-eight
exactly, without duplicates, wildcard recipes or optional omissions. Keep all
owners, custom denial, native suites, bounded projections and read requirements.
Negative tests cover exact membership, older versions, weakened permission/source
scope/budgets and the assembled gate. Older synthetic fixtures explicitly remove
v20-only recipes; this is not a production compatibility fallback.

Validate and commit requirements, ADR, architecture, machine contract and negative
tests before production enum/catalog changes. No declaration scaffold counts as
management behavior. Implementation then adds category-owned definitions and
independent core/feature/runtime/MCP checks for actions/fields/versions, category
isolation, secret exclusion, optional/empty/malformed data, text/list/global bounds
and safe audit. Run both available full host gates. No live router changes, real
configuration capture or remote publication. macOS/MSVC and hardware evidence stay
explicitly pending.

## Primary reference review

The pinned OpenWrt 25.12.5 sources inform field selection, not installed-state
assertions. The new reads never execute these scripts:

- [LED initialization](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/init.d/led): configuration and runtime trigger/brightness are distinct.
- [Dropbear defaults](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/dropbear/files/dropbear.config) and [validation](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/dropbear/files/dropbear.init): policy/interface fields are separate from keys and forced commands.
- [uHTTPd defaults](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/uhttpd/files/uhttpd.config): endpoint/timeout/security options versus credentials and handlers.
- [odhcpd defaults/migration](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/odhcpd/files/odhcpd.defaults): optional storage paths vary; observing them does not run migration.
