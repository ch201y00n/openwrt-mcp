//! Explicit synthetic downgrade only; never a production profile fallback.
pub fn before_v20(spec: &mut toml::Value) {
    before_v21(spec);
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

pub fn before_v21(spec: &mut toml::Value) {
    spec.as_table_mut()
        .unwrap()
        .remove("guarded_mutation_contract");
}
