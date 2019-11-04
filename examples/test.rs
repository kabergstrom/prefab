use prefab::{ComponentTypeUuid, EntityUuid, Prefab, PrefabUuid, Storage};
use serde::{Deserialize, Deserializer, Serialize};
use serde_diff::{Apply, SerdeDiff};
use std::cell::RefCell;
use type_uuid::TypeUuid;

#[derive(SerdeDiff, TypeUuid, Serialize, Deserialize, Debug, Clone)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct Transform {
    translation: Vec<f32>,
    scale: Vec<f32>,
}

const TEXT: &str = r#"Prefab(
    // Prefab AssetUuid
    id: "5fd8256d-db36-4fe2-8211-c7b3446e1927",
    objects: [
        // Inline definition of an entity and its components
        Entity(Entity(
             // Entity AssetUuid
             id: "62b3dbd1-56a8-469e-a262-41a66321da8b",
             // Component data and types
             components: [
                 (
                     // Component AssetTypeId
                     type: "d4b83227-d3f8-47f5-b026-db615fb41d31",
                     data: (
                         translation: [0.0, 0.0, 5.0],
                         scale: [2.0, 2.0, 2.0]
                     ),
                 ),
             ]
        )),
       // Embed the contents of another prefab in this prefab and override certain values
       PrefabRef((
             prefab_id: "14dec17f-ae14-40a3-8e44-e487fc423287",
             entity_overrides: [
                 (
                      entity_id: "030389e7-7ded-4d1a-aca3-d6912b19116c",
                      // Override values of a component in an entity of the referenced prefab
                      component_overrides: [
                          (
                              component_type: "d4b83227-d3f8-47f5-b026-db615fb41d31",
                              diff: [ Enter(Field("translation")), Enter(CollectionIndex(1)), Value(5.0) ]
                          ),
                      ],
                 ),
             ],
       ))
    ]
)"#;

struct World {
    transform: RefCell<Option<Transform>>,
}

impl prefab::Storage for World {
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
    fn add_prefab_ref<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        target_prefab: &PrefabUuid,
    ) {
    }
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
    let mut deserializer = ron::de::Deserializer::from_bytes(TEXT.as_bytes()).unwrap();
    let world = World {
        transform: RefCell::new(None),
    };
    let prefab_deserializer = prefab::PrefabDeserializer { storage: &world };
    <prefab::PrefabDeserializer<World> as serde::de::DeserializeSeed>::deserialize(
        prefab_deserializer,
        &mut deserializer,
    )
    .unwrap();
}
