//! Closed, pure sessionless rpcd UCI read recipes. Not transaction authority.
use crate::{
    Action, Category, Collection, CoreError, Operation, OutputMode, Permission, Presence,
    Requirement, ScalarKind, Selection, TypedProjection,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UciReadProfile {
    System,
    NetworkInterfaces,
    WirelessRadios,
    FirewallDefaults,
    Dnsmasq,
    Mounts,
}

impl UciReadProfile {
    pub const ALL: [Self; 6] = [
        Self::System,
        Self::NetworkInterfaces,
        Self::WirelessRadios,
        Self::FirewallDefaults,
        Self::Dnsmasq,
        Self::Mounts,
    ];

    pub const fn config(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::NetworkInterfaces => "network",
            Self::WirelessRadios => "wireless",
            Self::FirewallDefaults => "firewall",
            Self::Dnsmasq => "dhcp",
            Self::Mounts => "fstab",
        }
    }

    pub const fn section_type(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::NetworkInterfaces => "interface",
            Self::WirelessRadios => "wifi-device",
            Self::FirewallDefaults => "defaults",
            Self::Dnsmasq => "dnsmasq",
            Self::Mounts => "mount",
        }
    }

    pub const fn category(self) -> Category {
        match self {
            Self::System => Category::System,
            Self::NetworkInterfaces => Category::Network,
            Self::WirelessRadios => Category::Wireless,
            Self::FirewallDefaults => Category::Firewall,
            Self::Dnsmasq => Category::DhcpDns,
            Self::Mounts => Category::Storage,
        }
    }
}

pub(crate) fn validate_read(operation: &Operation) -> Result<(), CoreError> {
    let Action::Ubus {
        object,
        method,
        arguments,
    } = &operation.action
    else {
        return Ok(());
    };
    if object != "uci" {
        return Ok(());
    }
    let invalid = CoreError::InvalidDefinition;
    let profile = UciReadProfile::ALL
        .into_iter()
        .find(|profile| {
            arguments.get("config").and_then(serde_json::Value::as_str) == Some(profile.config())
                && arguments.get("type").and_then(serde_json::Value::as_str)
                    == Some(profile.section_type())
        })
        .ok_or(invalid)?;
    if method != "get"
        || arguments.len() != 2
        || !operation.parameters.is_empty()
        || !operation.requirements.contains(&Requirement {
            category: profile.category(),
            permission: Permission::Read,
        })
    {
        return Err(invalid);
    }
    let OutputMode::Typed(projection) = &operation.output_mode else {
        return Err(invalid);
    };
    let TypedProjection::Collection {
        reject_if_present,
        collection:
            Collection::ObjectEntries {
                source,
                max_items,
                key,
                record,
            },
        selection: Selection::All {},
    } = projection.as_ref()
    else {
        return Err(invalid);
    };
    if source != "/values"
        || !(1..=128).contains(max_items)
        || !(1..=256).contains(&key.max_bytes)
        || !record.collections.is_empty()
        || !reject_if_present.iter().any(|path| path == "/error")
        || !record.fields.iter().any(|field| {
            field.source == "/.type"
                && field.presence == Presence::Required
                && matches!(&field.value, ScalarKind::TextEnum { values, .. }
                    if values.len() == 1 && values[0] == profile.section_type())
        })
    {
        return Err(invalid);
    }
    Ok(())
}
