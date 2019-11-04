use atelier_core::asset_uuid;
use prefab::{ComponentTypeUuid, EntityUuid, Prefab, PrefabUuid, StorageDeserializer};
use serde::{Deserialize, Deserializer, Serialize};
use serde_diff::{Apply, SerdeDiff};
use std::{cell::RefCell, collections::HashMap};
use type_uuid::TypeUuid;
mod prefab_sample;

#[derive(SerdeDiff, TypeUuid, Serialize, Deserialize, Debug, Clone)]
#[uuid = "d4b83227-d3f8-47f5-b026-db615fb41d31"]
struct Transform {
    translation: Vec<f32>,
    scale: Vec<f32>,
}

struct ComponentMetadata {
    id: legion::storage::ComponentTypeId,
    comp_meta: legion::storage::ComponentMeta,
    data_pos: usize,
}
struct TagMetadata {
    id: legion::storage::TagTypeId,
    tag_meta: legion::storage::TagMeta,
    data_pos: usize,
}

struct EntityCreator {
    components: Vec<ComponentMetadata>,
    tags: Vec<TagMetadata>,
    data: Vec<u8>,
}
impl EntityCreator {
    pub fn add_component<T: legion::storage::Component>(&mut self, val: T) {
        let meta = legion::storage::ComponentMeta::of::<T>();
        let data_pos = self.add_data(val, meta.size);
        self.components.push(ComponentMetadata {
            id: legion::storage::ComponentTypeId::of::<T>(),
            comp_meta: meta,
            data_pos,
        });
    }
    pub fn add_tag<T: legion::storage::Tag>(&mut self, val: T) {
        let meta = legion::storage::TagMeta::of::<T>();
        let data_pos = self.add_data(val, meta.size);
        self.tags.push(TagMetadata {
            id: legion::storage::TagTypeId::of::<T>(),
            tag_meta: meta,
            data_pos,
        });
    }
    fn add_data<T>(&mut self, val: T, size: usize) -> usize {
        let current_pos = self.data.len();
        let remaining_space = self.data.capacity() - current_pos;
        let needed_capacity = size.saturating_sub(remaining_space);
        if needed_capacity > 0 {
            self.data.reserve(needed_capacity);
        }
        unsafe {
            let dst = self.data.as_mut_ptr().offset(current_pos as isize);
            std::ptr::write_unaligned(dst.cast(), &val as *const T);
            self.data.set_len(current_pos + size);
            std::mem::forget(val);
        }
        current_pos
    }
}

struct RegisteredComponent {
    deserialize_fn:
        fn(&mut dyn erased_serde::Deserializer, &mut legion::world::World, legion::entity::Entity),
    apply_diff:
        fn(&mut dyn erased_serde::Deserializer, &mut legion::world::World, legion::entity::Entity),
}

struct InnerWorld {
    world: legion::world::World,
    cmd_buffer: legion::command::CommandBuffer,
    current_entity: Option<legion::entity::Entity>,
    entity_map: HashMap<EntityUuid, legion::entity::Entity>,
    registered_components: HashMap<ComponentTypeUuid, RegisteredComponent>,
}

struct World {
    inner: RefCell<InnerWorld>,
}

impl prefab::StorageDeserializer for &World {
    fn begin_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid) {
        let mut this = self.inner.borrow_mut();
        let new_entity = this.world.insert((), vec![()])[0];
        this.current_entity = Some(new_entity);
        this.entity_map.insert(*entity, new_entity);
    }
    fn end_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid) {
        let mut this = &mut *self.inner.borrow_mut();
        this.current_entity = None;
        // this.cmd_buffer.write(&mut this.world);
    }
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
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
        let entity = this.current_entity.expect("no current_entity");
        (registered.deserialize_fn)(
            &mut erased_serde::Deserializer::erase(deserializer),
            &mut this.world,
            entity,
        );
        println!("deserialized component");
        Ok(())
    }
    fn begin_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid) {
        let prefab = PREFABS
            .iter()
            .filter(|p| &p.0 == target_prefab)
            .nth(0)
            .expect("failed to find prefab");
        println!("reading prefab {:?}", prefab.0);
        read_prefab(prefab.1, self);
    }
    fn end_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid) {}
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        parent_prefab: &PrefabUuid,
        prefab_ref: &PrefabUuid,
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

fn read_prefab(text: &str, world: &World) {
    let mut deserializer = ron::de::Deserializer::from_bytes(text.as_bytes()).unwrap();

    prefab::Prefab::deserialize(&mut deserializer, &world).unwrap();
}

fn main() {
    let universe = legion::world::Universe::new();
    let mut cmd_buffer = legion::command::CommandBuffer::default();
    cmd_buffer.block = Some(universe.allocator.lock().allocate());
    use std::iter::FromIterator;
    let world = World {
        inner: RefCell::new(InnerWorld {
            world: universe.create_world(),
            cmd_buffer,
            current_entity: None,
            entity_map: HashMap::new(),
            registered_components: HashMap::from_iter(vec![(
                Transform::UUID,
                RegisteredComponent {
                    deserialize_fn: |d, world, entity| {
                        let comp = erased_serde::deserialize::<Transform>(d)
                            .expect("failed to deserialize transform");
                        println!("deserialized {:#?}", comp);
                        world.add_component(entity, comp);
                    },
                    apply_diff: |d, world, entity| {
                        let mut comp = world
                            .get_component_mut::<Transform>(entity)
                            .expect("expected component data when diffing");
                        let mut comp: &mut Transform = &mut *comp;
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
}
