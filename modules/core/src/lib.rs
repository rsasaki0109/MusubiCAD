//! OpenCAD core types: stable IDs, units, errors, document metadata, and
//! transaction primitives.

pub mod document;
pub mod error;
pub mod id;
pub mod manifest;
pub mod serialize;
pub mod transaction;
pub mod units;
pub mod validation;

pub use document::{DocumentKind, DocumentMetadata};
pub use error::{OpenCadError, Result};
pub use id::{
    BodyId, ComponentId, ConnectorId, ConstraintId, DocumentId, EntityId, FeatureId, InstanceId,
    MateId, MaterialId, ParameterId, PatchId, PatternId, SheetId, SketchId, TopoRefId, ViewId,
};
pub use manifest::OcadManifest;
pub use serialize::{sha256_hex, sorted_map, to_pretty_json};
pub use transaction::{Transaction, TransactionAction, TransactionLog};
pub use units::{Angle, Density, DensityUnit, Expression, Length, LengthUnit, Mass, MassUnit};
pub use validation::{ValidationLevel, ValidationMessage, ValidationReport};
