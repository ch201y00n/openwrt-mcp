use openwrt_mcp_core::{
    PreparedAction,
    packages::{MAX_PACKAGE_SOURCE_BYTES, PackageProfile},
};
use openwrt_mcp_device_codec::{compile_action, encode_remote, packages::*};

const ROW: &str = "Package: libfixture-20260913\nVersion: 1:2~abc-r1\nArchitecture: aarch64_generic\nStatus: install hold,user unpacked\n";
const BANNER: &[u8] = b"opkg version 38eccbb1fd694d4798ac1baf88f9ba83d1eac616 (2024-10-16)\n";
fn valid(text: &str) -> bool {
    parse_opkg_status(text.as_bytes(), MAX_PACKAGE_SOURCE_BYTES).is_ok()
}

#[test]
fn opkg_recipe_cannot_initialize_config_or_select_other_files_and_banner_is_exact() {
    for (command, tail) in [
        (opkg_version_command(), ["/bin/opkg", "--version"]),
        (opkg_status_command(), ["/bin/cat", "/usr/lib/opkg/status"]),
    ] {
        assert_eq!(command.program(), "/usr/bin/env");
        assert_eq!(
            command.arguments(),
            [
                "-i",
                "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
                "LANG=C",
                tail[0],
                tail[1]
            ]
        );
        let remote = String::from_utf8(encode_remote(&command).unwrap()).unwrap();
        assert_eq!(
            remote,
            format!(
                "exec '/usr/bin/env' '-i' 'PATH=/usr/sbin:/usr/bin:/sbin:/bin' 'LANG=C' '{}' '{}'",
                tail[0], tail[1]
            )
        );
    }
    assert!(compile_action(&PreparedAction::OpkgStatusPage { cursor: None }).is_err());
    validate_opkg_version(BANNER).unwrap();
    for bad in [
        b"".as_slice(),
        &BANNER[..BANNER.len() - 1],
        b"opkg version 0.6.0\n",
        &[b'x'; 257],
        b"\xff",
    ] {
        assert!(validate_opkg_version(bad).is_err());
    }
    for bad in [
        String::from_utf8(BANNER.to_vec())
            .unwrap()
            .replace("38eccbb1", "38eccbb2"),
        format!("warning\n{}", String::from_utf8_lossy(BANNER)),
        format!("{}\n", String::from_utf8_lossy(BANNER)),
    ] {
        assert!(validate_opkg_version(bad.as_bytes()).is_err());
    }
}

#[test]
fn opkg_parser_selects_exact_fields_and_discards_bounded_unknown_continuations() {
    let text = format!(
        "{}Conffiles:\n /fixture/private/hash not-a-secret\nX-Custom: ignored-private-value\n\tcontinued-private-value\n\n",
        ROW.replace("Package:", "pAcKaGe:")
    );
    let observed = parse_opkg_status(text.as_bytes(), text.len()).unwrap();
    assert_eq!(observed.profile(), PackageProfile::Opkg38eccbb1RootStatus);
    let page = observed.page(&[1; 16], 0).unwrap();
    assert_eq!(
        page["items"][0],
        serde_json::json!({"name":"libfixture-20260913","version":"1:2~abc-r1","arch":"aarch64_generic","status":"install hold,user unpacked"})
    );
    let encoded = page.to_string();
    for excluded in ["Conffiles", "private", "hash", "X-Custom", "layer"] {
        assert!(!encoded.contains(excluded));
    }
    assert!(valid(""));
    let rows: String = (0..281)
        .map(|i| {
            format!(
                "{}\n",
                ROW.replace("libfixture-20260913", &format!("fixture{i:04}"))
            )
        })
        .collect();
    assert_eq!(
        parse_opkg_status(rows.as_bytes(), rows.len())
            .unwrap()
            .page(&[1; 16], 0)
            .unwrap()["captured_count"],
        281
    );
}

#[test]
fn opkg_parser_rejects_incomplete_duplicate_aliased_or_continued_selected_fields() {
    for bad in [
        ROW.to_owned(),
        format!("{ROW}garbage\n\n"),
        format!("{ROW}package: other\n\n"),
        format!("{ROW}X-A: one\nx-a: two\n\n"),
        format!("{ROW} continued\n\n"),
        format!("\tignored\n{ROW}\n"),
        format!("{ROW}\n{ROW}\n"),
        format!("{ROW}9header: value\n\n"),
        format!("{ROW}bad_name: value\n\n"),
        format!("{ROW}bad:missing-space\n\n"),
    ] {
        assert!(!valid(&bad), "bad framing accepted");
    }
    for name in ["Package", "Version", "Architecture", "Status"] {
        let lines: String = ROW
            .lines()
            .filter(|line| !line.starts_with(&format!("{name}:")))
            .map(|line| format!("{line}\n"))
            .collect();
        assert!(!valid(&format!("{lines}\n")));
        assert!(!valid(&format!("{lines}{name}:\n\n")));
        assert!(!valid(&format!("{lines}{name}: value\n\tcontinued\n\n")));
    }
    for control in ['\0', '\r', '\u{85}', '\u{7f}'] {
        assert!(!valid(&format!("{ROW}X-Ignored: before{control}after\n\n")));
    }
    assert!(parse_opkg_status(b"\xff\n\n", 100).is_err());
    assert!(!valid(&format!("{ROW}\nPackage: late-incomplete\n\n")));
}

#[test]
fn opkg_parser_line_field_record_and_raw_limits_are_independent() {
    let exact = format!("{ROW}X: {}\n\n", "x".repeat(8189));
    assert!(valid(&exact));
    assert!(!valid(&exact.replace("X: ", "X: xx")));
    let name = "N".repeat(64);
    assert!(valid(&format!("{ROW}{name}: value\n\n")));
    assert!(!valid(&format!("{ROW}{name}N: value\n\n")));
    let mut fields = ROW.to_owned();
    for i in 0..60 {
        fields.push_str(&format!("X-{i}: unused\n"));
    }
    assert!(valid(&format!("{fields}\n")));
    assert!(!valid(&format!("{fields}X-60: unused\n\n")));
    let rows: String = (0..4096)
        .map(|i| {
            format!(
                "{}\n",
                ROW.replace("libfixture-20260913", &format!("fixture{i:04}"))
            )
        })
        .collect();
    assert!(valid(&rows));
    assert!(!valid(&format!("{rows}{ROW}\n")));
    let text = format!("{ROW}\n");
    assert!(parse_opkg_status(text.as_bytes(), text.len()).is_ok());
    assert!(parse_opkg_status(text.as_bytes(), text.len() - 1).is_err());
    assert!(parse_opkg_status(&vec![b'\n'; MAX_PACKAGE_SOURCE_BYTES + 1], usize::MAX).is_err());
}
