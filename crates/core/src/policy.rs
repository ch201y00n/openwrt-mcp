use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{Catalog, CoreError, Operation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    System,
    Network,
    Wireless,
    Firewall,
    DhcpDns,
    Services,
    Packages,
    Storage,
    Vpn,
    Firmware,
    Diagnostics,
    Extensions,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    #[default]
    Deny,
    Read,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub category: Category,
    pub permission: Permission,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Grant {
    pub access: Access,
    pub execute: bool,
}

impl Grant {
    fn permits(self, permission: Permission) -> bool {
        if self.access == Access::Deny {
            return false;
        }
        match permission {
            Permission::Read => true,
            Permission::Write => self.access == Access::ReadWrite,
            Permission::Execute => self.execute,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub categories: BTreeMap<Category, Grant>,
    pub allow_operations: Option<BTreeSet<String>>,
    pub deny_operations: BTreeSet<String>,
}

impl Policy {
    /// Validate exact operation references after the complete catalog is built.
    pub fn validate(&self, catalog: &Catalog) -> Result<(), CoreError> {
        if self
            .deny_operations
            .iter()
            .chain(self.allow_operations.iter().flatten())
            .any(|name| catalog.get(name).is_none())
        {
            return Err(CoreError::UnknownOperationReference);
        }
        Ok(())
    }

    pub fn authorize(&self, operation: &Operation) -> Result<(), CoreError> {
        if operation.requirements.is_empty()
            || self.deny_operations.contains(&operation.name)
            || self
                .allow_operations
                .as_ref()
                .is_some_and(|allowed| !allowed.contains(&operation.name))
        {
            return Err(CoreError::PermissionDenied);
        }
        for requirement in &operation.requirements {
            let grant = self
                .categories
                .get(&requirement.category)
                .copied()
                .unwrap_or_default();
            if !grant.permits(requirement.permission) {
                return Err(CoreError::PermissionDenied);
            }
        }
        Ok(())
    }
}
