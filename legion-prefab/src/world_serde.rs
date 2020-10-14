use crate::format::EntityUuid;
use crate::registration::ComponentRegistration;
use legion::serialize::{EntitySerializer, UnknownType};
use legion::storage::{ArchetypeIndex, UnknownComponentStorage, UnknownComponentWriter};
use legion::{
    storage::{ComponentTypeId, EntityLayout},
    *,
};
use serde::{Deserialize, Deserializer, Serializer};
use std::{cell::RefCell, collections::HashMap};

pub struct CustomSerializer<'a> {
    pub comp_types: &'a HashMap<ComponentTypeId, ComponentRegistration>,
    pub entity_map: RefCell<&'a mut HashMap<legion::Entity, EntityUuid>>,
}

impl<'a> legion::serialize::EntitySerializer for CustomSerializer<'a> {
    fn serialize(
        &self,
        entity: Entity,
        serialize_fn: &mut dyn FnMut(&dyn erased_serde::Serialize),
    ) {
        let mut entity_map = self.entity_map.borrow_mut();

        let uuid = entity_map
            .entry(entity)
            .or_insert_with(|| *uuid::Uuid::new_v4().as_bytes());
        serialize_fn(&uuid::Uuid::from_bytes(*uuid));
    }
    fn deserialize(
        &self,
        _deserializer: &mut dyn erased_serde::Deserializer,
    ) -> Result<Entity, erased_serde::Error> {
        panic!("CustomSerializer can only be used to serialize")
    }
}

impl<'a> legion::serialize::WorldSerializer for CustomSerializer<'a> {
    type TypeId = type_uuid::Bytes;

    fn map_id(
        &self,
        type_id: ComponentTypeId,
    ) -> Result<Self::TypeId, legion::serialize::UnknownType> {
        let uuid = self.comp_types.get(&type_id).map(|x| *x.uuid());

        match uuid {
            Some(uuid) => Ok(uuid),
            None => Err(legion::serialize::UnknownType::Error),
        }
    }

    unsafe fn serialize_component<S: Serializer>(
        &self,
        ty: ComponentTypeId,
        ptr: *const u8,
        serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> {
        if let Some(reg) = self.comp_types.get(&ty) {
            let mut result = None;
            let mut serializer = Some(serializer);

            // The safety is guaranteed due to the guarantees of the registration,
            // namely that the ComponentTypeId maps to a ComponentRegistration of
            // the correct type.
            reg.comp_serialize(ptr, &mut |serialize| {
                result.replace(erased_serde::serialize(
                    serialize,
                    serializer.take().unwrap(),
                ));
            });

            result.take().unwrap()
        } else {
            panic!("serialize_component received unserializable type {:?}", ty);
        }
    }

    unsafe fn serialize_component_slice<S: Serializer>(
        &self,
        ty: ComponentTypeId,
        storage: &dyn UnknownComponentStorage,
        archetype: ArchetypeIndex,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if let Some(reg) = self.comp_types.get(&ty) {
            let mut serializer = Some(serializer);
            let mut result = None;
            let result_ref = &mut result;
            reg.comp_serialize_slice(storage, archetype, &mut move |serializable| {
                *result_ref = Some(erased_serde::serialize(
                    serializable,
                    serializer
                        .take()
                        .expect("serialize can only be called once"),
                ));
            });
            result.unwrap()
        } else {
            panic!(
                "serialize_component_slice received unserializable type {:?}",
                ty
            );
        }
    }

    fn with_entity_serializer(
        &self,
        callback: &mut dyn FnMut(&dyn EntitySerializer),
    ) {
        callback(self)
    }
}

pub struct CustomDeserializer<'a> {
    pub comp_types_uuid: &'a HashMap<type_uuid::Bytes, ComponentRegistration>,
    pub comp_types: &'a HashMap<ComponentTypeId, ComponentRegistration>,
    pub entity_map: RefCell<&'a mut HashMap<EntityUuid, Entity>>,
    pub allocator: RefCell<legion::world::Allocate>,
}

impl<'a> legion::serialize::EntitySerializer for CustomDeserializer<'a> {
    fn serialize(
        &self,
        _entity: Entity,
        _serialize_fn: &mut dyn FnMut(&dyn erased_serde::Serialize),
    ) {
        panic!("Cannot serialize with CustomDeserializer")
    }
    fn deserialize(
        &self,
        deserializer: &mut dyn erased_serde::Deserializer,
    ) -> Result<Entity, erased_serde::Error> {
        let entity_uuid = <uuid::Uuid as Deserialize>::deserialize(deserializer)?;
        let mut entity_map = self.entity_map.borrow_mut();
        let entity = entity_map
            .entry(*entity_uuid.as_bytes())
            .or_insert_with(|| self.allocator.borrow_mut().next().unwrap());
        Ok(*entity)
    }
}

impl<'r> legion::serialize::WorldDeserializer for CustomDeserializer<'r> {
    type TypeId = type_uuid::Bytes;

    fn unmap_id(
        &self,
        type_id: &Self::TypeId,
    ) -> Result<ComponentTypeId, UnknownType> {
        let uuid = self
            .comp_types_uuid
            .get(type_id)
            .map(|x| x.component_type_id());

        match uuid {
            Some(component_type_id) => Ok(component_type_id),
            None => Err(legion::serialize::UnknownType::Error),
        }
    }

    fn register_component(
        &self,
        type_id: Self::TypeId,
        layout: &mut EntityLayout,
    ) {
        self.comp_types_uuid
            .get(&type_id)
            .unwrap()
            .register_component(layout);
    }

    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        deserializer: D,
    ) -> Result<Box<[u8]>, <D as Deserializer<'de>>::Error> {
        use serde::de::Error;
        let mut erased = erased_serde::Deserializer::erase(deserializer);
        if let Some(reg) = self.comp_types.get(&type_id) {
            reg.comp_deserialize(&mut erased).map_err(D::Error::custom)
        } else {
            panic!(
                "deserialize_component received undeserializable type {:?}",
                type_id
            );
        }
    }

    fn deserialize_component_slice<'a, 'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        writer: UnknownComponentWriter<'a>,
        deserializer: D,
    ) -> Result<(), D::Error> {
        if let Some(reg) = self.comp_types.get(&type_id) {
            use serde::de::Error;
            let mut deserializer = erased_serde::Deserializer::erase(deserializer);
            reg.comp_deserialize_slice(writer, &mut deserializer)
                .map_err(D::Error::custom)
        } else {
            panic!(
                "deserialize_component_slice received undeserializable type {:?}",
                type_id
            );
        }
    }

    fn with_entity_serializer(
        &self,
        callback: &mut dyn FnMut(&dyn EntitySerializer),
    ) {
        callback(self)
    }
}
