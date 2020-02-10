#[doc(hidden)]
pub use inventory;

use prefab_format as format;

mod registration;
pub use registration::{
    ComponentRegistration, TagRegistration, iter_component_registrations, iter_tag_registrations, DiffSingleResult
};

mod prefab_serde;
pub use prefab_serde::{
    ComponentOverride, PrefabRef, PrefabMeta, Prefab, PrefabFormatDeserializer, PrefabSerdeContext,
    PrefabFormatSerializer,
};

mod cooked_prefab;
pub use cooked_prefab::{CookedPrefab};

mod world_serde;
pub use world_serde::{SerializeImpl, DeserializeImpl};
