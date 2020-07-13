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
use legion::storage::{UnknownComponentStorage, ArchetypeIndex};
use serde::de::DeserializeSeed;


pub struct CustomSerializer {
    pub comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
}

impl legion::serialize::WorldSerializer for CustomSerializer {
    type TypeId = type_uuid::Bytes;

    fn map_id(&self, type_id: ComponentTypeId) -> Option<Self::TypeId> {
        self.comp_types.get(&type_id).and_then(|x| Some(*x.uuid()))
    }

    unsafe fn serialize_component<S: Serializer>(&self, ty: ComponentTypeId, ptr: *const u8, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error> {
        if let Some(reg) = self.comp_types.get(&ty) {
            let mut result = None;
            let mut serializer = Some(serializer);

            // The safety is guaranteed due to the guarantees of the registration,
            // namely that the ComponentTypeId maps to a ComponentRegistration of
            // the correct type.
            unsafe {
                reg.comp_serialize(ptr, &mut |serialize| {
                    result.replace(erased_serde::serialize(
                        serialize,
                        serializer.take().unwrap(),
                    ));
                });
            }

            return result.take().unwrap();
        }
        panic!(
            "received unserializable type {:?}",
            ty
        );
    }
}


pub struct CustomDeserializer {
    pub comp_types_uuid: HashMap<type_uuid::Bytes, ComponentRegistration>,
    pub comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
}

impl legion::serialize::WorldDeserializer for CustomDeserializer {
    type TypeId = type_uuid::Bytes;

    fn unmap_id(&self, type_id: &Self::TypeId) -> Option<ComponentTypeId> {
        self.comp_types_uuid.get(type_id).map(|x| x.component_type_id())
    }

    fn register_component(&self, type_id: Self::TypeId, layout: &mut EntityLayout) {
        self.comp_types_uuid.get(&type_id).unwrap().register_component(layout);
    }

    fn deserialize_insert_component<'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        storage: &mut UnknownComponentStorage,
        arch_index: ArchetypeIndex,
        deserializer: D
    ) -> Result<(), <D as Deserializer<'de>>::Error> {
        use serde::de::Error;
        let mut erased = erased_serde::Deserializer::erase(deserializer);
        let reg = self.comp_types.get(&type_id).unwrap();
        reg.comp_deserialize(storage, arch_index, &mut erased).map_err(D::Error::custom)
    }

    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        deserializer: D
    ) -> Result<Box<[u8]>, <D as Deserializer<'de>>::Error> {
        use serde::de::Error;
        let mut erased = erased_serde::Deserializer::erase(deserializer);
        let reg = self.comp_types.get(&type_id).unwrap();
        reg.deserialize_single(&mut erased).map_err(D::Error::custom)
    }
}

pub struct CustomDeserializerSeed<'a> {
    pub deserializer: &'a CustomDeserializer,
    pub universe: &'a Universe,
}

impl<'de, 'a> DeserializeSeed<'de> for CustomDeserializerSeed<'a> {
    type Value = World;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
    {
        let wrapped = legion::serialize::UniverseDeserializerWrapper(self.deserializer, self.universe);
        wrapped.deserialize(deserializer)
    }
}
