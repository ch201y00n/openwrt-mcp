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
    Timeservers,
    NetworkDevices,
    BridgeVlans,
    RoutesV4,
    RoutesV6,
    RulesV4,
    RulesV6,
    FirewallZones,
    FirewallForwardings,
    FirewallRules,
    FirewallRedirects,
    FirewallNats,
    DhcpPools,
    DhcpHosts,
    DnsDomains,
    DnsCnames,
    StorageGlobals,
    Swaps,
}

impl UciReadProfile {
    pub const ALL: [Self; 24] = [
        Self::System,
        Self::NetworkInterfaces,
        Self::WirelessRadios,
        Self::FirewallDefaults,
        Self::Dnsmasq,
        Self::Mounts,
        Self::Timeservers,
        Self::NetworkDevices,
        Self::BridgeVlans,
        Self::RoutesV4,
        Self::RoutesV6,
        Self::RulesV4,
        Self::RulesV6,
        Self::FirewallZones,
        Self::FirewallForwardings,
        Self::FirewallRules,
        Self::FirewallRedirects,
        Self::FirewallNats,
        Self::DhcpPools,
        Self::DhcpHosts,
        Self::DnsDomains,
        Self::DnsCnames,
        Self::StorageGlobals,
        Self::Swaps,
    ];

    pub const fn config(self) -> &'static str {
        match self {
            Self::System | Self::Timeservers => "system",
            Self::NetworkInterfaces
            | Self::NetworkDevices
            | Self::BridgeVlans
            | Self::RoutesV4
            | Self::RoutesV6
            | Self::RulesV4
            | Self::RulesV6 => "network",
            Self::WirelessRadios => "wireless",
            Self::FirewallDefaults
            | Self::FirewallZones
            | Self::FirewallForwardings
            | Self::FirewallRules
            | Self::FirewallRedirects
            | Self::FirewallNats => "firewall",
            Self::Dnsmasq
            | Self::DhcpPools
            | Self::DhcpHosts
            | Self::DnsDomains
            | Self::DnsCnames => "dhcp",
            Self::Mounts | Self::StorageGlobals | Self::Swaps => "fstab",
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
            Self::Timeservers => "timeserver",
            Self::NetworkDevices => "device",
            Self::BridgeVlans => "bridge-vlan",
            Self::RoutesV4 => "route",
            Self::RoutesV6 => "route6",
            Self::RulesV4 | Self::FirewallRules => "rule",
            Self::RulesV6 => "rule6",
            Self::FirewallZones => "zone",
            Self::FirewallForwardings => "forwarding",
            Self::FirewallRedirects => "redirect",
            Self::FirewallNats => "nat",
            Self::DhcpPools => "dhcp",
            Self::DhcpHosts => "host",
            Self::DnsDomains => "domain",
            Self::DnsCnames => "cname",
            Self::StorageGlobals => "global",
            Self::Swaps => "swap",
        }
    }

    pub const fn category(self) -> Category {
        match self {
            Self::System | Self::Timeservers => Category::System,
            Self::NetworkInterfaces
            | Self::NetworkDevices
            | Self::BridgeVlans
            | Self::RoutesV4
            | Self::RoutesV6
            | Self::RulesV4
            | Self::RulesV6 => Category::Network,
            Self::WirelessRadios => Category::Wireless,
            Self::FirewallDefaults
            | Self::FirewallZones
            | Self::FirewallForwardings
            | Self::FirewallRules
            | Self::FirewallRedirects
            | Self::FirewallNats => Category::Firewall,
            Self::Dnsmasq
            | Self::DhcpPools
            | Self::DhcpHosts
            | Self::DnsDomains
            | Self::DnsCnames => Category::DhcpDns,
            Self::Mounts | Self::StorageGlobals | Self::Swaps => Category::Storage,
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
        || record
            .collections
            .iter()
            .any(|field| !matches!(field.collection, Collection::TextOption { .. }))
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
