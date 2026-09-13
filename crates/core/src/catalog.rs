use std::collections::{BTreeMap, BTreeSet};

use crate::{Category, CoreError, Operation, OutputMode, Permission, Requirement};

const MAX_CUSTOM_OPERATIONS: usize = 1024;

/// Immutable, validated operation metadata owned by the server operator.
#[derive(Debug, Clone)]
pub struct Catalog {
    operations: Vec<Operation>,
    index: BTreeMap<String, usize>,
}

impl Catalog {
    /// Construct an extension-only catalog. Device features are injected by the caller.
    pub fn new(custom: Vec<Operation>) -> Result<Self, CoreError> {
        Self::with_builtins(Vec::new(), custom)
    }

    /// Install reviewed feature definitions and privileged operator extensions.
    pub fn with_builtins(
        builtins: Vec<Operation>,
        custom: Vec<Operation>,
    ) -> Result<Self, CoreError> {
        if custom.len() > MAX_CUSTOM_OPERATIONS {
            return Err(CoreError::CatalogLimitExceeded);
        }
        if builtins
            .iter()
            .any(|operation| matches!(operation.output_mode, OutputMode::Structured))
        {
            return Err(CoreError::InvalidDefinition);
        }
        let mut operations = builtins;
        let mut names: BTreeSet<String> = operations
            .iter()
            .map(|operation| operation.name.clone())
            .collect();
        if names.len() != operations.len() {
            return Err(CoreError::DuplicateOperation);
        }
        for mut operation in custom {
            // Introspection of uci must not grant operator extensions a raw
            // configuration/credential or mutation path, even when privileged.
            if matches!(&operation.action, crate::Action::Ubus { object, .. } if object == "uci") {
                return Err(CoreError::InvalidDefinition);
            }
            if !names.insert(operation.name.clone()) {
                return Err(CoreError::DuplicateOperation);
            }
            // Validate before injection so extensions must declare their own
            // affected categories and cannot omit all permissions.
            operation.validate()?;
            for permission in [Permission::Write, Permission::Execute] {
                let required = Requirement {
                    category: Category::Extensions,
                    permission,
                };
                if !operation.requirements.contains(&required) {
                    operation.requirements.push(required);
                }
            }
            operation.validate()?;
            operations.push(operation);
        }
        for operation in &operations {
            operation.validate()?;
        }
        let index = operations
            .iter()
            .enumerate()
            .map(|(index, operation)| (operation.name.clone(), index))
            .collect();
        Ok(Self { operations, index })
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn get(&self, name: &str) -> Option<&Operation> {
        self.index.get(name).map(|index| &self.operations[*index])
    }
}
