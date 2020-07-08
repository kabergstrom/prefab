use crate::registration::ComponentRegistration;
use legion::{
    world::Allocate,
    *,
    storage::{
        EntityLayout, ComponentMeta, ComponentStorage, ComponentTypeId,
    },
};
use serde::{de::IgnoredAny, Deserialize, Deserializer, Serialize, Serializer};
use std::{cell::RefCell, collections::HashMap, ptr::NonNull};
//
// #[derive(Serialize, Deserialize)]
// struct SerializedEntityLayout {
//     component_types: Vec<type_uuid::Bytes>,
// }
//
// pub struct SerializeImpl {
//     comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
//     entity_map: RefCell<HashMap<Entity, uuid::Bytes>>,
// }
//
// impl SerializeImpl {
//     pub fn new(
//         comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
//         entity_map: HashMap<Entity, uuid::Bytes>,
//     ) -> Self {
//         SerializeImpl {
//             comp_types,
//             entity_map: RefCell::new(entity_map),
//         }
//     }
//
//     pub fn take_entity_map(self) -> HashMap<Entity, uuid::Bytes> {
//         self.entity_map.into_inner()
//     }
// }
//
// impl legion::serialize::ser::WorldSerializer for SerializeImpl {
//
//     fn can_serialize_component(
//         &self,
//         ty: &ComponentTypeId,
//         _meta: &ComponentMeta,
//     ) -> bool {
//         self.comp_types.get(&ty).is_some()
//     }
//     fn serialize_entity_layout<S: Serializer>(
//         &self,
//         serializer: S,
//         entity_layout: &EntityLayout,
//     ) -> Result<S::Ok, S::Error> {
//         let components_to_serialize = entity_layout
//             .components()
//             .iter()
//             .filter_map(|(ty, _)| self.comp_types.get(&ty))
//             .map(|reg| reg.uuid)
//             .collect::<Vec<_>>();
//         SerializedEntityLayout {
//             component_types: components_to_serialize,
//         }
//         .serialize(serializer)
//     }
//     fn serialize_components<S: Serializer>(
//         &self,
//         serializer: S,
//         component_type: &ComponentTypeId,
//         _component_meta: &ComponentMeta,
//         components: &ComponentResourceSet,
//     ) -> Result<S::Ok, S::Error> {
//         if let Some(reg) = self.comp_types.get(&component_type) {
//             let result = RefCell::new(None);
//             let serializer = RefCell::new(Some(serializer));
//             {
//                 let mut result_ref = result.borrow_mut();
//                 // The safety is guaranteed due to the guarantees of the registration,
//                 // namely that the ComponentTypeId maps to a ComponentRegistration of
//                 // the correct type.
//                 unsafe {
//                     (reg.comp_serialize_fn)(components, &mut |serialize| {
//                         result_ref.replace(erased_serde::serialize(
//                             serialize,
//                             serializer.borrow_mut().take().unwrap(),
//                         ));
//                     });
//                 }
//             }
//             return result.borrow_mut().take().unwrap();
//         }
//         panic!(
//             "received unserializable type {:?}, this should be filtered by can_serialize",
//             component_type
//         );
//     }
//
//     fn serialize_entities<S: Serializer>(
//         &self,
//         serializer: S,
//         entities: &[Entity],
//     ) -> Result<S::Ok, S::Error> {
//         let mut uuid_map = self.entity_map.borrow_mut();
//         serializer.collect_seq(entities.iter().map(|e| {
//             *uuid_map
//                 .entry(*e)
//                 .or_insert_with(|| *uuid::Uuid::new_v4().as_bytes())
//         }))
//     }
// }

// pub struct DeserializeImpl {
//     pub comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
//     pub comp_types_by_uuid: HashMap<type_uuid::Bytes, ComponentRegistration>,
//     pub entity_map: RefCell<HashMap<uuid::Bytes, Entity>>,
// }
// impl DeserializeImpl {
//     pub fn new(
//         tag_types: HashMap<TagTypeId, TagRegistration>,
//         comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
//     ) -> Self {
//         use std::iter::FromIterator;
//         Self {
//             comp_types_by_uuid: HashMap::from_iter(
//                 comp_types.iter().map(|(_, val)| (val.uuid, val.clone())),
//             ),
//             comp_types,
//             entity_map: RefCell::new(HashMap::new()),
//         }
//     }
// }
/*
impl legion::serialize::de::WorldDeserializer for DeserializeImpl {
    fn deserialize_entity_layout<'de, D: Deserializer<'de>>(
        &self,
        deserializer: D,
    ) -> Result<EntityLayout, <D as Deserializer<'de>>::Error> {
        let serialized_desc =
            <SerializedEntityLayout as Deserialize>::deserialize(deserializer)?;
        let mut desc = EntityLayout::default();
        for tag in serialized_desc.tag_types {
            if let Some(reg) = self.tag_types_by_uuid.get(&tag) {
                (reg.register_tag_fn)(&mut desc);
            }
        }
        for comp in serialized_desc.component_types {
            if let Some(reg) = self.comp_types_by_uuid.get(&comp) {
                (reg.register_comp_fn)(&mut desc);
            }
        }
        Ok(desc)
    }
    fn deserialize_components<'de, D: Deserializer<'de>>(
        &self,
        deserializer: D,
        component_type: &ComponentTypeId,
        _component_meta: &ComponentMeta,
        get_next_storage_fn: &mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
    ) -> Result<(), <D as Deserializer<'de>>::Error> {
        if let Some(reg) = self.comp_types.get(&component_type) {
            let mut erased = erased_serde::Deserializer::erase(deserializer);
            (reg.comp_deserialize_fn)(&mut erased, get_next_storage_fn)
                .map_err(<<D as serde::Deserializer<'de>>::Error as serde::de::Error>::custom)?;
        } else {
            <IgnoredAny>::deserialize(deserializer)?;
        }
        Ok(())
    }
    fn deserialize_tags<'de, D: Deserializer<'de>>(
        &self,
        deserializer: D,
        tag_type: &TagTypeId,
        _tag_meta: &TagMeta,
        tags: &mut TagStorage,
    ) -> Result<(), <D as Deserializer<'de>>::Error> {
        if let Some(reg) = self.tag_types.get(&tag_type) {
            let mut erased = erased_serde::Deserializer::erase(deserializer);
            (reg.tag_deserialize_fn)(&mut erased, tags)
                .map_err(<<D as serde::Deserializer<'de>>::Error as serde::de::Error>::custom)?;
        } else {
            <IgnoredAny>::deserialize(deserializer)?;
        }
        Ok(())
    }
    fn deserialize_entities<'de, D: Deserializer<'de>>(
        &self,
        deserializer: D,
        entity_allocator: &EntityAllocator,
        entities: &mut Vec<Entity>,
    ) -> Result<(), <D as Deserializer<'de>>::Error> {
        let entity_uuids = <Vec<uuid::Bytes> as Deserialize>::deserialize(deserializer)?;
        let mut entity_map = self.entity_map.borrow_mut();
        for id in entity_uuids {
            let entity = entity_allocator.create_entity();
            entity_map.insert(id, entity);
            entities.push(entity);
        }
        Ok(())
    }
}
*/