//! LuCI-visible storage observations, not complete device inventories.
use crate::definition::read;
use openwrt_mcp_core::{
    Category, Collection, CounterSource, InnerRecord, Operation, OutputMode, Presence, ScalarField,
    ScalarKind, Selection, TextIdentity, TypedProjection,
};

pub(crate) fn operations() -> Vec<Operation> {
    vec![mounts(), block_devices()]
}

fn text(name: &str, source: &str, max_bytes: usize, presence: Presence) -> ScalarField {
    ScalarField {
        name: name.into(),
        source: source.into(),
        presence,
        value: ScalarKind::Text { max_bytes },
    }
}

fn bytes(name: &str, source: &str) -> ScalarField {
    ScalarField {
        name: name.into(),
        source: source.into(),
        presence: Presence::Required,
        value: ScalarKind::DecimalCounter {
            source: CounterSource::JsonInteger,
        },
    }
}

fn mounts() -> Operation {
    let mut operation = read(
        "storage_mounts",
        "Read up to 128 LuCI-visible mount/device paths and decimal-text capacity bytes. Non-atomic; failed/zero-block statvfs entries are omitted upstream, so empty is not proof of absence. May contact remote filesystems; no mount changes or file contents.",
        Category::Storage,
        "luci",
        "getMountPoints",
        "storage_mounts.v1",
        &[],
    );
    operation.output_mode = OutputMode::Typed(Box::new(TypedProjection::Collection {
        collection: Collection::ObjectArray {
            source: "/result".into(),
            max_items: 128,
            identity: "mount".into(),
            record: InnerRecord {
                fields: vec![
                    text("mount", "/mount", 1024, Presence::Required),
                    text("device", "/device", 1024, Presence::Required),
                    bytes("size_bytes", "/size"),
                    bytes("available_bytes", "/avail"),
                    bytes("free_bytes", "/free"),
                ],
                collections: vec![],
            },
        },
        selection: Selection::All {},
    }));
    operation
}

fn block_devices() -> Operation {
    let mut operation = read(
        "storage_block_devices",
        "Read up to 128 LuCI-visible block/swap entries: source key, device path, decimal-text size, filesystem and optional UUID/label/version/mount. Non-atomic; empty/partial upstream results do not prove absence. Signature reads may spin up disks; no activation, repairs or file contents.",
        Category::Storage,
        "luci",
        "getBlockDevices",
        "storage_block_devices.v1",
        &[],
    );
    operation.output_mode = OutputMode::Typed(Box::new(TypedProjection::Collection {
        collection: Collection::ObjectEntries {
            source: String::new(),
            max_items: 128,
            key: TextIdentity {
                name: "source_key".into(),
                max_bytes: 1024,
            },
            record: InnerRecord {
                fields: vec![
                    text("device", "/dev", 1024, Presence::Required),
                    bytes("size_bytes", "/size"),
                    text("filesystem", "/type", 64, Presence::Required),
                    text("uuid", "/uuid", 256, Presence::Optional),
                    text("label", "/label", 256, Presence::Optional),
                    text("version", "/version", 64, Presence::Optional),
                    text("mount", "/mount", 1024, Presence::Optional),
                ],
                collections: vec![],
            },
        },
        selection: Selection::All {},
    }));
    operation
}
