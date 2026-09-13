//! Explicit synthetic downgrade only; never a production profile fallback.
pub fn before_v20(spec: &mut toml::Value) {
    spec["uci_read_contract"]["profiles"]
        .as_array_mut()
        .unwrap()
        .retain(|p| {
            !matches!(
                p.as_str(),
                Some("system:led" | "dropbear:dropbear" | "uhttpd:uhttpd" | "dhcp:odhcpd")
            )
        });
}
