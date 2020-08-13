use atelier_core::asset_uuid;
use prefab_format::{ComponentTypeUuid, EntityUuid, PrefabUuid};
use serde::{Deserialize, Deserializer, Serialize};
use std::{cell::RefCell, collections::HashMap};
use type_uuid::TypeUuid;
use serde_diff::SerdeDiff;
use legion::*;
mod prefab_sample {
    include!("prefab_sample.rs.inc");
}

#[derive(SerdeDiff, TypeUuid, Serialize, Deserialize, Debug, Clone)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct Transform {
    value: u32,
    translation: Vec<f32>,
    scale: Vec<f32>,
}

struct RegisteredComponent {
    deserialize_fn:
        fn(&mut dyn erased_serde::Deserializer, &mut legion::world::World, legion::Entity),
    apply_diff:
        fn(&mut dyn erased_serde::Deserializer, &mut legion::world::World, legion::Entity),
}

struct InnerWorld {
    world: legion::world::World,
    entity_map: HashMap<EntityUuid, legion::Entity>,
    registered_components: HashMap<ComponentTypeUuid, RegisteredComponent>,
}

struct World {
    inner: RefCell<InnerWorld>,
}

impl prefab_format::StorageDeserializer for &World {
    fn begin_prefab(
        &self,
        _prefab: &PrefabUuid,
    ) {
    }
    fn begin_entity_object(
        &self,
        _prefab: &PrefabUuid,
        entity: &EntityUuid,
    ) {
        let mut this = self.inner.borrow_mut();
        let new_entity = this.world.extend(vec![()])[0];
        this.entity_map.insert(*entity, new_entity);
    }
    fn end_entity_object(
        &self,
        _prefab: &PrefabUuid,
        _entity: &EntityUuid,
    ) {
    }
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        _prefab: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        println!("deserializing transform");
        let mut this = self.inner.borrow_mut();
        let registered = this
            .registered_components
            .get(component_type)
            .expect("failed to find component type");
        let entity = *this
            .entity_map
            .get(entity)
            .expect("could not find prefab ref entity");
        (registered.deserialize_fn)(
            &mut erased_serde::Deserializer::erase(deserializer),
            &mut this.world,
            entity,
        );
        println!("deserialized component");
        Ok(())
    }
    fn begin_prefab_ref(
        &self,
        _prefab: &PrefabUuid,
        target_prefab: &PrefabUuid,
    ) {
        let prefab = PREFABS
            .iter()
            .filter(|p| &p.0 == target_prefab)
            .nth(0)
            .expect("failed to find prefab");
        println!("reading prefab {:?}", prefab.0);
        read_prefab(prefab.1, self);
    }
    fn end_prefab_ref(
        &self,
        _prefab: &PrefabUuid,
        _target_prefab: &PrefabUuid,
    ) {
    }
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        _parent_prefab: &PrefabUuid,
        _prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        let mut this = self.inner.borrow_mut();
        let registered = this
            .registered_components
            .get(component_type)
            .expect("failed to find component type");
        let entity = *this
            .entity_map
            .get(entity)
            .expect("could not find prefab ref entity");
        println!("applying diff");
        (registered.apply_diff)(
            &mut erased_serde::Deserializer::erase(deserializer),
            &mut this.world,
            entity,
        );
        Ok(())
    }
}

const PREFABS: [(PrefabUuid, &'static str); 2] = [
    (
        asset_uuid!("5fd8256d-db36-4fe2-8211-c7b3446e1927").0,
        prefab_sample::PREFAB1,
    ),
    (
        asset_uuid!("14dec17f-ae14-40a3-8e44-e487fc423287").0,
        prefab_sample::PREFAB2,
    ),
];

fn read_prefab(
    text: &str,
    world: &World,
) {
    let mut deserializer = ron::de::Deserializer::from_bytes(text.as_bytes()).unwrap();

    prefab_format::deserialize(&mut deserializer, &world).unwrap();
}

fn main() {
    use std::iter::FromIterator;
    let world = World {
        inner: RefCell::new(InnerWorld {
            world: legion::World::default(),
            entity_map: HashMap::new(),
            registered_components: HashMap::from_iter(vec![(
                Transform::UUID,
                RegisteredComponent {
                    deserialize_fn: |d, world, entity| {
                        let comp = erased_serde::deserialize::<Transform>(d)
                            .expect("failed to deserialize transform");
                        println!("deserialized {:#?}", comp);
                        world.entry(entity).unwrap().add_component(comp);
                    },
                    apply_diff: |d, world, entity| {
                        let mut e = world
                            .entry(entity)
                            .unwrap();

                        let comp = e.get_component_mut::<Transform>()
                            .expect("expected component data when diffing");
                        let comp: &mut Transform = &mut *comp;
                        println!("before diff {:#?}", comp);
                        <serde_diff::Apply<Transform> as serde::de::DeserializeSeed>::deserialize(
                            serde_diff::Apply::deserializable(comp),
                            d,
                        )
                        .expect("failed to deserialize diff");
                        println!("after diff {:#?}", comp);
                    },
                },
            )]),
        }),
    };
    read_prefab(PREFABS[0].1, &world);
    println!("done!");
}
