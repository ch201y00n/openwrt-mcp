# Closed UCI configuration observations (v11/v12)

These six parameterless tools read fixed `uci.get` config/type pairs under their own category's Read grant. No Execute flag is required. All custom Ubus `uci` operations are rejected, including privileged extensions. A discovered setter does not become callable.

The [reference rpcd implementation](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/uci.c) selects the shared `/tmp/.uci` delta view without a session. Results are **sessionless shared-delta, non-atomic observations**, not committed-only files, another LuCI session's staging or effective service state. The configured SSH authority and MCP policy protect these reads; no session is manufactured and no fallback is attempted. Missing package/object/method or incompatible output fails safely, not as an empty success.

Network interface and dnsmasq configuration use `<tool>.v2` for the text/list fields below; the other four response IDs remain `<tool>.v1`. The result is `{items:[...]}`, at most 128 unique section-map keys from `/values`. Each row requires `section` (map key, 1..256 UTF-8 bytes), `section_type` (exact expected `/.type`), `anonymous` (`/.anonymous`, boolean), and `index` (`/.index`, integer 0..4294967295). Item order is map order, not UCI section order; retain the index without inferring contiguous values. Neither identifier is a durable mutation handle. Any root `/error`, wrong section type, malformed field/container, invalid metadata or exceeded bound rejects the whole response.

Only the optional scalar text fields below are selected. Names are unchanged; parentheses give UTF-8 byte limits. Missing stays absent; empty text stays empty. Values such as `0`, `1`, `auto` and numeric strings are **not** interpreted, validated as option semantics or replaced by guessed defaults. Lists, booleans, numbers, null and objects at these option paths fail instead of coercion. All global collection, response byte and decoder limits remain in force.

| Tool / category | config / type | Optional text options |
| --- | --- | --- |
| system_configuration / system | system / system | hostname (256), timezone (256), zonename (256) |
| network_interface_configuration / network | network / interface | proto (64), device (256), mtu (32), metric (32), auto (8), defaultroute (8), peerdns (8), delegate (8), ip4table (64), ip6table (64) |
| wireless_radio_configuration / wireless | wireless / wifi-device | type (64), path (256), macaddr (17), disabled (8), country (8), channel (32), htmode (32), band (16), txpower (32) |
| firewall_defaults_configuration / firewall | firewall / defaults | input (32), output (32), forward (32), synflood_protect (8), drop_invalid (8), flow_offloading (8), flow_offloading_hw (8), disable_ipv6 (8) |
| dhcp_dnsmasq_configuration / dhcp_dns | dhcp / dnsmasq | domain (256), local (256), port (32), cachesize (32), expandhosts (8), domainneeded (8), boguspriv (8), rebind_protection (8), noresolv (8), localservice (8), authoritative (8), strictorder (8), logqueries (8) |
| storage_mount_configuration / storage | fstab / mount | device (1024), uuid (256), label (256), target (1024), fstype (64), enabled (8), enabled_fsck (8) |

## v12 text/list fields

Only these additional optional options accept a string **or** a list of strings. Every option is a terminal TextOption collection with max_items=128 and the per-text UTF-8 limit below. The limits apply to the whole original string too, not to guessed tokens within it.

| Tool (response v2) | Additional options |
| --- | --- |
| network_interface_configuration | ipaddr (1024), ip6addr (1024), dns (1024), ifname (256) |
| dhcp_dnsmasq_configuration | server (1024), address (1024), interface (256), notinterface (256), rebind_domain (1024), addnhosts (1024) |

A string becomes `{"kind":"string","values":["original text"]}` and a list becomes `{"kind":"list","values":["first","second"]}`. No splitting/joining, trimming, deduplication, host/path resolution or syntax validation. Missing options stay absent; empty string and empty list remain distinct. Duplicate and empty list values are preserved. Nested/mixed lists, null, non-text scalars, NUL and exceeded bounds reject the entire response. Existing scalar options above still reject lists.

Containing section rows and **all** option values share the 256-item scan/emission budget. Each original string counts as one value; every duplicate list value counts separately. The 8-node schema limit and 64-KiB normalized/256-KiB MCP result limits are unchanged. A large but otherwise legitimate configuration may exceed this deliberately bounded observation; there is no silent truncation or unbounded fallback.

These are partial, intentionally reviewed projections, not raw configuration export. Wireless BSS/SSID/keys, firewall rules/zones, DHCP reservations, arbitrary mount options (which can contain passwords), commands, VPN keys and every undeclared field are excluded. Selected addresses, resolver/domain/interface settings, paths/UUIDs/MACs/hostnames may be sensitive; grant access accordingly. addnhosts exposes configured paths only, not file contents. An untrusted target could place arbitrary text in an allowed field; field selection is not a guarantee that device-controlled text contains no secrets. Historical v11 emulator evidence does not validate these new v2 fields.

No configuration mutation, service reload, package installation, backup/restore, mount operation or live router acceptance is implied. Synthetic tests are separate from emulator and physical-target evidence.
