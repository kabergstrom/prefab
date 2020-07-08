#[doc(hidden)]
pub use inventory;

use prefab_format as format;

mod registration;
pub use registration::{
    ComponentRegistration, iter_component_registrations,
    DiffSingleResult,
};

mod prefab_uncooked;
pub use prefab_uncooked::{
    ComponentOverride, PrefabRef, PrefabMeta, Prefab, PrefabFormatDeserializer, PrefabSerdeContext,
    PrefabFormatSerializer,
};

mod prefab_cooked;
pub use prefab_cooked::{CookedPrefab};

mod prefab_builder;
pub use prefab_builder::PrefabBuilder;
pub use prefab_builder::PrefabBuilderError;

mod world_serde;
//pub use world_serde::{SerializeImpl, DeserializeImpl};

mod cooking;
pub use cooking::cook_prefab;

// Implements a safer, easier to use layer on top of legion's clone_from and clone_from_single by
// using the type registry in legion-prefab
mod clone_merge;
pub use clone_merge::CopyCloneImpl;
pub use clone_merge::SpawnCloneImpl;
pub use clone_merge::SpawnCloneImplHandlerSet;
pub use clone_merge::SpawnFrom;
pub use clone_merge::SpawnInto;
