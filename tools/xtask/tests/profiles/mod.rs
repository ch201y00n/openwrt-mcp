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
    before_v22(spec);
    spec.as_table_mut()
        .unwrap()
        .remove("guarded_mutation_contract");
}

pub fn before_v22(spec: &mut toml::Value) {
    spec.as_table_mut()
        .unwrap()
        .remove("ciphertext_foundation_contract");
    for rule in spec["crates"].as_array_mut().unwrap() {
        let name = rule["name"].as_str().unwrap().to_owned();
        rule["dependencies"].as_array_mut().unwrap().retain(|d| {
            !matches!(
                (name.as_str(), d.as_str()),
                ("openwrt-mcp-crypto-age", Some("hmac" | "sha2"))
                    | (
                        "openwrt-mcp-adapters" | "openwrt-mcp-backend-ssh",
                        Some("zeroize")
                    )
                    | ("openwrt-mcp-key-sources", Some("async-trait"))
            )
        });
    }
}
