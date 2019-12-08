use serde::{Deserialize, Serialize};
use serde_diff::SerdeDiff;
use type_uuid::TypeUuid;

// Components require TypeUuid + Serialize + Deserialize + SerdeDiff + Send + Sync
#[derive(TypeUuid, Serialize, Deserialize, SerdeDiff)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct Transform {
    value: u32,
    translation: Vec<f32>,
    scale: Vec<f32>,
}

legion_prefab::register_component_type!(Transform);

// Tags require Clone + PartialEq in addition to the component requirements, but not SerdeDiff
#[derive(TypeUuid, Serialize, Deserialize, Clone, PartialEq)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct ModelId(u32);

legion_prefab::register_tag_type!(ModelId);

fn main() {}
