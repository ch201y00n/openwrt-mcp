use openwrt_mcp_core::{CoreError, packages::*};
use serde_json::json;

fn row(i: usize) -> OpkgStatusRecord {
    OpkgStatusRecord {
        name: format!("libfixture{i:04}-20260913"),
        version: "1:2~abc-r1".into(),
        arch: "aarch64_generic".into(),
        status: "install hold,user unpacked".into(),
    }
}

#[test]
fn opkg_rows_keep_exact_recorded_status_without_apk_layer_or_installed_filter() {
    let observation =
        PackageObservation::new_opkg_status((0..281).rev().map(row).collect()).unwrap();
    assert_eq!(
        observation.profile(),
        PackageProfile::Opkg38eccbb1RootStatus
    );
    let mut page = observation.page(&[1; 16], 0).unwrap();
    let mut names = vec![];
    loop {
        assert_eq!(page["source"], "opkg_38eccbb1");
        assert_eq!(page["scope"], "opkg_root_status_file");
        assert_eq!(page["whole_device_complete"], false);
        assert_eq!(page["consistency"], "non_atomic_observation");
        assert_eq!(page["captured_count"], 281);
        let items = page["items"].as_array().unwrap();
        assert!(items.len() <= 16);
        for item in items {
            assert_eq!(item.as_object().unwrap().len(), 4);
            assert_eq!(item["status"], "install hold,user unpacked");
            assert_eq!(item["arch"], "aarch64_generic");
            assert_eq!(item["version"], "1:2~abc-r1");
            assert!(item.get("layer").is_none());
            names.push(item["name"].as_str().unwrap().to_owned());
        }
        let Some(next) = page.get("next_cursor") else {
            break;
        };
        let cursor = PackageCursor::parse(next.as_str().unwrap()).unwrap();
        page = observation.page(cursor.nonce(), cursor.offset()).unwrap();
    }
    assert_eq!(names, (0..281).map(|i| row(i).name).collect::<Vec<_>>());
    let empty = PackageObservation::new_opkg_status(vec![])
        .unwrap()
        .page(&[1; 16], 0)
        .unwrap();
    assert_eq!(empty["items"], json!([]));
    assert!(empty.get("next_cursor").is_none());
    assert_eq!(
        PackageObservation::new(vec![]).unwrap().profile(),
        PackageProfile::Apk3_0_5
    );
}

#[test]
fn opkg_row_field_and_retained_limits_include_status_and_reject_late_duplicates() {
    assert!(PackageObservation::new_opkg_status((0..4096).map(row).collect()).is_ok());
    assert!(matches!(
        PackageObservation::new_opkg_status((0..4097).map(row).collect()),
        Err(CoreError::OutputLimit)
    ));
    for field in 0..4 {
        let bound = [256, 256, 64, 128][field];
        for text in [
            String::new(),
            "x".repeat(bound + 1),
            "é".repeat(bound / 2 + 1),
            "bad\ntext".into(),
            "bad\ttext".into(),
            "bad\u{85}text".into(),
            "bad\0text".into(),
        ] {
            let mut rows: Vec<_> = (0..281).map(row).collect();
            let tail = &mut rows[280];
            *match field {
                0 => &mut tail.name,
                1 => &mut tail.version,
                2 => &mut tail.arch,
                _ => &mut tail.status,
            } = text;
            assert!(PackageObservation::new_opkg_status(rows).is_err());
        }
    }
    let mut duplicates: Vec<_> = (0..281).map(row).collect();
    duplicates[280].name = duplicates[0].name.clone();
    assert!(PackageObservation::new_opkg_status(duplicates).is_err());
    let exact: Vec<_> = (0..4096)
        .map(|i| OpkgStatusRecord {
            name: format!("{i:04}{}", "n".repeat(252)),
            version: "v".repeat(128),
            arch: "a".repeat(64),
            status: "s".repeat(64),
        })
        .collect();
    assert!(PackageObservation::new_opkg_status(exact.clone()).is_ok());
    let mut overflow = exact;
    overflow[4095].status.push('s');
    assert!(matches!(
        PackageObservation::new_opkg_status(overflow),
        Err(CoreError::OutputLimit)
    ));
}

#[test]
fn opkg_worst_case_pages_keep_bounded_serialization_and_canonical_cursors() {
    let rows = (0..17)
        .map(|i| OpkgStatusRecord {
            name: format!("{i:02}{}", "\\".repeat(254)),
            version: "\"".repeat(256),
            arch: "é".repeat(32),
            status: "\\".repeat(128),
        })
        .collect();
    let observation = PackageObservation::new_opkg_status(rows).unwrap();
    let page = observation.page(&[255; 16], 0).unwrap();
    assert!(serde_json::to_vec(&page).unwrap().len() <= 65536);
    assert_eq!(page["items"][0]["status"], "\\".repeat(128));
    let next = PackageCursor::parse(page["next_cursor"].as_str().unwrap()).unwrap();
    assert_eq!(next.offset(), 16);
    assert_eq!(
        observation.page(next.nonce(), 16).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for (nonce, offset) in [([0; 16], 0), ([1; 16], 1), ([1; 16], 32)] {
        assert!(observation.page(&nonce, offset).is_err());
    }
}
