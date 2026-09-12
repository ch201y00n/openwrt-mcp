use openwrt_mcp::config::Config;

const ENVIRONMENT_ONLY: &str = r#"
[protection]
cipher = "age"
recipient_source = "public"

[protection.sources.public]
kind = "environment"
variable = "OPENWRT_MCP_SYNTHETIC_ABSENT_PUBLIC_4C0E"
"#;

#[test]
fn legacy_configs_have_no_key_authority_and_offline_checks_never_read_sources() {
    let legacy: Config = toml::from_str("").unwrap();
    assert!(legacy.protection.is_none());
    let configured: Config = toml::from_str(ENVIRONMENT_ONLY).unwrap();
    configured.catalog().unwrap();
    let protection = configured.protection.unwrap();
    let encrypt = protection.encryption().unwrap();
    assert_eq!(encrypt.format_id(), "age-x25519-v1");
    assert!(protection.decryption().unwrap().is_none());
}

#[test]
fn configured_identity_is_separate_and_is_not_opened_during_wiring() {
    let text = ENVIRONMENT_ONLY.replace(
        "recipient_source = \"public\"",
        "recipient_source = \"public\"\nidentity_source = \"private\"",
    ) + "\n[protection.sources.private]\nkind = 'environment'\nvariable = 'OPENWRT_MCP_SYNTHETIC_ABSENT_PRIVATE_4C0E'\n";
    let config: Config = toml::from_str(&text).unwrap();
    config.catalog().unwrap();
    let protection = config.protection.unwrap();
    protection.encryption().unwrap();
    assert_eq!(
        protection.decryption().unwrap().unwrap().format_id(),
        "age-x25519-v1"
    );
}

#[test]
fn cipher_typo_inline_keys_and_unknown_options_are_rejected() {
    for text in [
        ENVIRONMENT_ONLY.replace("cipher = \"age\"", "cipher = \"custom\""),
        ENVIRONMENT_ONLY.replace(
            "cipher = \"age\"",
            "cipher = \"age\"\nprivate_key = \"synthetic-private-value\"",
        ),
        ENVIRONMENT_ONLY.to_owned() + "secret = 'synthetic-private-value'\n",
        ENVIRONMENT_ONLY.to_owned() + "\n[protection.crypto_limits]\nunknown = 1\n",
        ENVIRONMENT_ONLY.replace("cipher = \"age\"", "cipher = \"age\"\nkey_limits = []"),
        ENVIRONMENT_ONLY.replace("cipher = \"age\"", "cipher = \"age\"\ncrypto_limits = []"),
    ] {
        assert!(toml::from_str::<Config>(&text).is_err());
    }
}

#[test]
fn unknown_alias_purpose_reuse_and_invalid_limits_fail_offline() {
    for text in [
        ENVIRONMENT_ONLY.replace(
            "recipient_source = \"public\"",
            "recipient_source = \"missing\"",
        ),
        ENVIRONMENT_ONLY.replace(
            "recipient_source = \"public\"",
            "recipient_source = \"public\"\nidentity_source = \"public\"",
        ),
        ENVIRONMENT_ONLY.to_owned() + "\n[protection.crypto_limits]\nmax_input_bytes = 0\n",
        ENVIRONMENT_ONLY.to_owned() + "\n[protection.key_limits]\nmax_entries = 0\n",
    ] {
        let config: Config = toml::from_str(&text).unwrap();
        assert_eq!(config.catalog().err(), Some("protection_config_invalid"));
    }
}
