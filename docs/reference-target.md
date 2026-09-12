# Initial reference target

This is a point-in-time read-only observation, not a continuing assertion of live router state. No configuration, credentials, MACs, public addresses, decrypted backups or key material are included. Do not use this document instead of refreshing capabilities on an actual management session.

## Observed software identity

Observed through the operator's existing strict-host-key SSH connection at **2026-09-12T19:12:44Z** (2026-09-13 Asia/Seoul), using the read-only system board endpoint and retaining only these fields:

| Field | Observed value |
| --- | --- |
| Distribution | OpenWrt |
| Release | 25.12.5 |
| Revision | r33051-f5dae5ece4 |
| Target | mediatek/filogic |
| Kernel | 6.12.94 |
| Model | Banana Pi BPI-R4 (2x SFP+) |
| Board | bananapi,bpi-r4 |

## Selected installed package observations

Observed at **2026-09-12T19:16:08Z** using package-manager version and installed-package metadata only. The manager reported apk-tools 3.0.5 for aarch64. The following subset drives initial compatibility work; it is not a complete device image manifest.

| Package/family | Observed version |
| --- | --- |
| ubus / libubus | 2026.06.28~24864e78-r2 |
| rpcd, file/iwinfo/rpcsys/ucode modules | 2026.07.19~e37ed9d8-r1 |
| uci / libuci | 2025.12.02~66127cd7-r1 |
| netifd | 2026.02.26~cbb83a18-r2 |
| procd | 2026.03.13~58eb263d-r1 |
| firewall4 | 2025.03.17~b6e51575-r2 |
| nftables-json | 1.1.6-r2 |
| dnsmasq | 2.93-r1 |
| odhcpd-ipv6only | 2026.06.29~5d7be43f-r1 |
| dropbear | 2025.89-r3 |
| wpad-basic-mbedtls / hostapd-common | 2025.08.26~ca266cc2-r2 |
| iw | 6.17-r1 |
| uhttpd / uhttpd-mod-ubus | 2026.06.16~7b1bec45-r1 |
| wireguard-tools | 1.0.20250521-r1 |
| tailscale | 1.98.3-r1 |
| samba4-server | 4.22.7-r3 |
| nginx-ssl | 1.26.3-r4 |
| avahi-dbus-daemon | 0.9_rc5-r1 |
| adblock | 4.5.7-r4 |
| acme-acmesh / DNS API | 3.1.3-r3 |

The installed rpcd revision differs from the base 25.12.5 package definition. Thus a release string alone cannot identify all relevant APIs or security fixes. Its full upstream revision is [e37ed9d814699098eb7e26c8b33c054840782dfb](https://github.com/openwrt/rpcd/commit/e37ed9d814699098eb7e26c8b33c054840782dfb); the preceding release-base revision is [28faf6403792d25b9826043aaf37880624c19568](https://github.com/openwrt/rpcd/commit/28faf6403792d25b9826043aaf37880624c19568). Source equality was checked for UCI/session logic, not assumed from version numbers.

## Visible management API metadata

At **2026-09-12T19:18:27Z**, read-only ubus introspection returned 60 visible object headers. Only selected common object/method names were retained; no method with state-changing behavior was called. Common visible objects included system, service, uci, file, iwinfo, luci, luci-rpc, network, network.device, network.interface, network.wireless, rc, rpc-sys and dhcp.

Relevant observations include uci apply/confirm/rollback/reload_config; rc list/init; iwinfo devices/info/freqlist/countrylist/survey; network.interface dump/status; luci getMountPoints/getBlockDevices/getFeatures; rpc-sys packagelist/upgrade_test; system validate_firmware_image/sysupgrade. Presence is **not** permission, a safe effect classification, or proof of successful behavior. Omitted methods can be hidden by remote ACLs and must not automatically be declared absent.

Hardware-specific Wi-Fi, DSA, SFP, boot/flash and KT IPTV preservation require separately authorized hardware acceptance. Generic ARM64 QEMU can test userspace contracts but cannot establish those hardware properties.
