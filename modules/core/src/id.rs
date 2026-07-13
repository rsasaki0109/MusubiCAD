use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{OpenCadError, Result};

macro_rules! define_id {
    ($name:ident, $prefix:literal) => {
        /// Stable, semantic-prefixed identifier.
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;

            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                Self::validate(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            fn validate(value: &str) -> Result<()> {
                let expected = concat!($prefix, ":");
                if !value.starts_with(expected) {
                    return Err(OpenCadError::InvalidId(format!(
                        "id '{value}' must start with '{expected}'"
                    )));
                }
                let suffix = &value[expected.len()..];
                if suffix.is_empty() {
                    return Err(OpenCadError::InvalidId(format!(
                        "id '{value}' must have a non-empty suffix after prefix"
                    )));
                }
                if !suffix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '/'))
                {
                    return Err(OpenCadError::InvalidId(format!(
                        "id '{value}' contains invalid characters"
                    )));
                }
                Ok(())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = OpenCadError;

            fn from_str(s: &str) -> Result<Self> {
                Self::new(s)
            }
        }
    };
}

define_id!(DocumentId, "doc");
define_id!(ParameterId, "param");
define_id!(SketchId, "sketch");
define_id!(FeatureId, "feature");
define_id!(BodyId, "body");
define_id!(MaterialId, "mat");
define_id!(EntityId, "ent");
define_id!(ConstraintId, "con");
define_id!(TopoRefId, "ref");
define_id!(ComponentId, "component");
define_id!(InstanceId, "instance");
define_id!(MateId, "mate");
define_id!(ConnectorId, "connector");
define_id!(PatternId, "pattern");
define_id!(SheetId, "sheet");
define_id!(ViewId, "view");
define_id!(DimensionId, "dim");
define_id!(PatchId, "patch");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_ids_accepted() {
        assert!(DocumentId::new("doc:bracket_001").is_ok());
        assert!(FeatureId::new("feature:extrude_base").is_ok());
        assert!(TopoRefId::new("ref:face:base_top").is_ok());
    }

    #[test]
    fn invalid_prefix_rejected() {
        assert!(DocumentId::new("feature:wrong").is_err());
    }

    #[test]
    fn empty_suffix_rejected() {
        assert!(SketchId::new("sketch:").is_err());
    }
}
