use openwrt_mcp_core::{PreparedAction, packages::MAX_PACKAGE_SOURCE_BYTES};
use openwrt_mcp_device_codec::{compile_action, encode_remote, packages::*};
use serde_json::json;
mod opkg;
#[test]
fn recipe_clears_configuration_and_never_expands_pattern() {
    let command = apk_installed_command();
    assert_eq!(command.program(), "/usr/bin/env");
    assert_eq!(
        command.arguments(),
        [
            "-i",
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            "LANG=C",
            "APK_CONFIG=/dev/null",
            "/usr/bin/apk",
            "--no-network",
            "--no-cache",
            "--no-logfile",
            "query",
            "--from",
            "installed",
            "--installed",
            "--all-matches",
            "--format",
            "json",
            "--fields",
            "name,version,arch,layer",
            "*"
        ]
    );
    let remote = String::from_utf8(encode_remote(&command).unwrap()).unwrap();
    assert!(remote.ends_with(" '*'") && remote.starts_with("exec '/usr/bin/env' '-i'"));
    assert!(compile_action(&PreparedAction::ApkInstalledPage { cursor: None }).is_err());
    assert_eq!(
        apk_version_command().arguments().last().unwrap(),
        "--version"
    );
}
#[test]
fn version_requires_exact_banner_not_substring_or_release_guess() {
    for arch in ["aarch64", "x86_64", "aarch64_generic"] {
        validate_apk_version(format!("apk-tools 3.0.5, compiled for {arch}.\n").as_bytes())
            .unwrap();
    }
    for banner in [
        "apk-tools 3.0.6, compiled for aarch64.\n",
        "apk-tools 3.0.50, compiled for aarch64.\n",
        "WARNING\napk-tools 3.0.5, compiled for aarch64.\n",
        "apk-tools 3.0.5, compiled for .\n",
        "apk-tools 3.0.5, compiled for aarch64.\nextra",
        "apk-tools 3.0.5, compiled for aarch64.",
    ] {
        assert!(validate_apk_version(banner.as_bytes()).is_err());
    }
    assert!(validate_apk_version(&[b'x'; 257]).is_err());
}
#[test]
fn strict_decoder_rejects_duplicate_unknown_null_and_late_malformed_data() {
    for bytes in [
        br#"[{"name":"a","name":"b","version":"1","arch":"a"}]"#.as_slice(),
        br#"[{"name":"a","version":"1","arch":"a","secret":"hidden"}]"#,
        br#"[{"name":"a","version":"1","arch":"a","layer":null}]"#,
        br#"[{"name":"a","version":1,"arch":"a"}]"#,
        b"null",
        b"",
        b"[] trailing",
    ] {
        assert!(parse_apk_installed(bytes, 65536).is_err());
    }
    let mut rows: Vec<_> = (0..281)
        .map(|i| json!({"name":format!("fixture{i}"),"version":"1","arch":"a"}))
        .collect();
    rows[280]["layer"] = json!(2);
    assert!(parse_apk_installed(&serde_json::to_vec(&rows).unwrap(), 65536).is_err());
}
#[test]
fn source_limit_is_independent_of_pages_and_omitted_layer_means_root() {
    let bytes = br#"[{"name":"libfixture-20260913","version":"1~abc-r2","arch":"aarch64"}]"#;
    let page = parse_apk_installed(bytes, bytes.len())
        .unwrap()
        .page(&[1; 16], 0)
        .unwrap();
    assert_eq!(page["items"][0]["layer"], 0);
    assert_eq!(page["items"][0]["name"], "libfixture-20260913");
    assert!(parse_apk_installed(bytes, bytes.len() - 1).is_err());
    let mut huge = b"[]".to_vec();
    huge.resize(MAX_PACKAGE_SOURCE_BYTES + 1, b' ');
    assert!(parse_apk_installed(&huge, 16 * 1024 * 1024).is_err());
}
