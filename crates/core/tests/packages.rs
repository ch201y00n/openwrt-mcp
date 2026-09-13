use openwrt_mcp_core::{CoreError, packages::*};
use serde_json::json;
mod opkg;
fn row(i: usize) -> PackageRecord {
    PackageRecord {
        name: format!("libfixture{i:04}-20260913"),
        version: "1~abc-r2".into(),
        arch: "aarch64".into(),
        layer: 0,
    }
}
#[test]
fn all_281_records_are_enumerated_in_stable_small_pages() {
    let observation = PackageObservation::new((0..281).rev().map(row).collect()).unwrap();
    let mut page = observation.page(&[1; 16], 0).unwrap();
    let mut names = vec![];
    loop {
        assert_eq!(page["captured_count"], 281);
        assert_eq!(page["whole_device_complete"], false);
        assert_eq!(page["scope"], "apk_query_installed_visible");
        assert_eq!(page["consistency"], "non_atomic_observation");
        let items = page["items"].as_array().unwrap();
        assert!(items.len() <= 16);
        names.extend(items.iter().map(|r| r["name"].as_str().unwrap().to_owned()));
        let Some(cursor) = page.get("next_cursor") else {
            break;
        };
        let cursor = PackageCursor::parse(cursor.as_str().unwrap()).unwrap();
        assert_eq!(cursor.nonce(), &[1; 16]);
        page = observation.page(cursor.nonce(), cursor.offset()).unwrap();
    }
    assert_eq!(names, (0..281).map(|i| row(i).name).collect::<Vec<_>>());
}
#[test]
fn empty_exact_limit_and_duplicate_layer_identities() {
    let empty = PackageObservation::new(vec![])
        .unwrap()
        .page(&[1; 16], 0)
        .unwrap();
    assert_eq!(empty["items"], json!([]));
    assert!(empty.get("next_cursor").is_none());
    assert!(PackageObservation::new((0..4096).map(row).collect()).is_ok());
    assert!(matches!(
        PackageObservation::new((0..4097).map(row).collect()),
        Err(CoreError::OutputLimit)
    ));
    assert!(PackageObservation::new(vec![row(0), row(0)]).is_err());
    let mut other = row(0);
    other.layer = 1;
    let page = PackageObservation::new(vec![other, row(0)])
        .unwrap()
        .page(&[2; 16], 0)
        .unwrap();
    assert_eq!(page["items"][0]["layer"], 0);
    assert_eq!(page["items"][1]["layer"], 1);
}
#[test]
fn late_bad_rows_field_bounds_and_retained_memory_limit_fail() {
    for bad in ["", "bad\nname", "bad\0name", "bad\u{0085}name"] {
        let mut rows: Vec<_> = (0..281).map(row).collect();
        rows[280].name = bad.into();
        assert!(PackageObservation::new(rows).is_err());
    }
    for field in 0..4 {
        let mut bad = row(0);
        match field {
            0 => bad.name = "x".repeat(257),
            1 => bad.version = "x".repeat(257),
            2 => bad.arch = "x".repeat(65),
            _ => bad.layer = 2,
        }
        assert!(PackageObservation::new(vec![bad]).is_err());
    }
    let rows = (0..4096)
        .map(|i| PackageRecord {
            name: format!("{i:04}{}", "x".repeat(252)),
            version: "v".repeat(256),
            arch: "a".repeat(64),
            layer: 0,
        })
        .collect();
    assert!(matches!(
        PackageObservation::new(rows),
        Err(CoreError::OutputLimit)
    ));
}
#[test]
fn worst_case_page_is_bounded_and_cursors_are_canonical() {
    let rows = (0..17)
        .map(|i| PackageRecord {
            name: format!("{i:02}{}", "\\".repeat(254)),
            version: "\"".repeat(256),
            arch: "a".repeat(64),
            layer: 0,
        })
        .collect();
    let observation = PackageObservation::new(rows).unwrap();
    let page = observation.page(&[255; 16], 0).unwrap();
    assert!(serde_json::to_vec(&page).unwrap().len() < 65536);
    let good = page["next_cursor"].as_str().unwrap();
    assert_eq!(PackageCursor::parse(good).unwrap().offset(), 16);
    for suffix in [
        "0", "01", "016", "+16", "-16", "1", "4096", "16.0", "16 ", "",
    ] {
        assert!(PackageCursor::parse(&format!("{}.{}", "ff".repeat(16), suffix)).is_err());
    }
    assert!(PackageCursor::parse(&good.to_uppercase()).is_err());
    assert!(PackageCursor::parse(&format!("{}.16", "00".repeat(16))).is_err());
    for (nonce, offset) in [([0; 16], 0), ([1; 16], 1), ([1; 16], 32)] {
        assert!(observation.page(&nonce, offset).is_err());
    }
}
