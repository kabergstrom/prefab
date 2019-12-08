use prefab_format::{self, ComponentTypeUuid, EntityUuid, PrefabUuid, StorageDeserializer};
use serde::{Deserialize, Deserializer, Serialize};
use serde_diff::{Apply, SerdeDiff};
use std::cell::RefCell;
use type_uuid::TypeUuid;
mod prefab_sample {
    include!("prefab_sample.rs.inc");
}

#[derive(SerdeDiff, TypeUuid, Serialize, Deserialize, Debug, Clone)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct Transform {
    translation: Vec<f32>,
    scale: Vec<f32>,
}

struct World {
    transform: RefCell<Option<Transform>>,
}

impl prefab_format::StorageDeserializer for World {
    fn begin_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid) {}
    fn end_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid) {}
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        println!("deserializing transform");
        *self.transform.borrow_mut() = Some(<Transform as Deserialize>::deserialize(deserializer)?);
        println!("deserialized {:?}", self.transform);
        Ok(())
    }
    fn begin_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid) {}
    fn end_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid) {}
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        parent_prefab: &PrefabUuid,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        let mut transform = self.transform.borrow_mut();
        let transform = transform.as_mut().expect("diff but value didn't exist");
        println!("applying diff");
        let before = transform.clone();
        Apply::apply(deserializer, &mut *transform)?;
        println!("before {:#?} after {:#?}", before, transform);
        Ok(())
    }
}

fn main() {
    let mut deserializer =
        ron::de::Deserializer::from_bytes(prefab_sample::PREFAB2.as_bytes()).unwrap();
    let world = World {
        transform: RefCell::new(None),
    };
    prefab_format::deserialize(&mut deserializer, &world).unwrap();
}
