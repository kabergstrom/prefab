use prefab::{ComponentTypeUuid, EntityUuid, Prefab, PrefabUuid, StorageDeserializer};
use serde::{Deserialize, Deserializer, Serialize};
use serde_diff::{Apply, SerdeDiff};
use std::cell::RefCell;
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

struct World {
    transform: RefCell<Option<Transform>>,
}

impl prefab::StorageDeserializer for World {
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
        ron::de::Deserializer::from_bytes(prefab_sample::TEXT.as_bytes()).unwrap();
    let world = World {
        transform: RefCell::new(None),
    };
    prefab::Prefab::deserialize(&mut deserializer, &world).unwrap();
}
