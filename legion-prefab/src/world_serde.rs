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
use legion::storage::{UnknownComponentStorage, ArchetypeIndex, UnknownComponentWriter};
use serde::de::DeserializeSeed;
use legion::serialize::{EntitySerializer, UnknownType};


pub struct CustomSerializer {
    pub comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
}

impl legion::serialize::EntitySerializerSource for CustomSerializer {
    fn entity_serializer(&self) -> Option<&parking_lot::Mutex<Box<dyn EntitySerializer>>> {
        //Some(&self.canon)
        unimplemented!();
    }
}

impl legion::serialize::WorldSerializer for CustomSerializer {
    type TypeId = type_uuid::Bytes;

    fn map_id(&self, type_id: ComponentTypeId) -> Result<Self::TypeId, legion::serialize::UnknownType> {
        let uuid = self.comp_types
            .get(&type_id)
            .and_then(|x| Some(*x.uuid()));

        match uuid {
            Some(uuid) => Ok(uuid),
            None => Err(legion::serialize::UnknownType::Error)
        }
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

    unsafe fn serialize_component_slice<S: Serializer>(
        &self,
        ty: ComponentTypeId,
        storage: &dyn UnknownComponentStorage,
        archetype: ArchetypeIndex,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        unimplemented!();
        // if let Some((_, serialize_fn, _, _, _)) = self.serialize_fns.get(&ty) {
        //     let mut serializer = Some(serializer);
        //     let mut result = None;
        //     let result_ref = &mut result;
        //     (serialize_fn)(storage, archetype, &mut move |serializable| {
        //         *result_ref = Some(erased_serde::serialize(
        //             serializable,
        //             serializer
        //                 .take()
        //                 .expect("serialize can only be called once"),
        //         ));
        //     });
        //     result.unwrap()
        // } else {
        //     panic!();
        // }
    }
}


pub struct CustomDeserializer {
    pub comp_types_uuid: HashMap<type_uuid::Bytes, ComponentRegistration>,
    pub comp_types: HashMap<ComponentTypeId, ComponentRegistration>,
}

impl legion::serialize::EntitySerializerSource for CustomDeserializer {
    fn entity_serializer(&self) -> Option<&parking_lot::Mutex<Box<dyn EntitySerializer>>> {
        //Some(&self.canon)
        unimplemented!();
    }
}

impl legion::serialize::WorldDeserializer for CustomDeserializer {
    type TypeId = type_uuid::Bytes;

    fn unmap_id(&self, type_id: &Self::TypeId) -> Result<ComponentTypeId, UnknownType> {
        //self.comp_types_uuid.get(type_id).map(|x| x.component_type_id())

        let uuid = self.comp_types_uuid
            .get(type_id)
            .and_then(|x| Some(x.component_type_id()));

        match uuid {
            Some(component_type_id) => Ok(component_type_id),
            None => Err(legion::serialize::UnknownType::Error)
        }
    }

    fn register_component(&self, type_id: Self::TypeId, layout: &mut EntityLayout) {
        self.comp_types_uuid.get(&type_id).unwrap().register_component(layout);
    }

    // fn deserialize_insert_component<'de, D: Deserializer<'de>>(
    //     &self,
    //     type_id: ComponentTypeId,
    //     storage: &mut UnknownComponentStorage,
    //     arch_index: ArchetypeIndex,
    //     deserializer: D
    // ) -> Result<(), <D as Deserializer<'de>>::Error> {
    //     use serde::de::Error;
    //     let mut erased = erased_serde::Deserializer::erase(deserializer);
    //     let reg = self.comp_types.get(&type_id).unwrap();
    //     reg.comp_deserialize(storage, arch_index, &mut erased).map_err(D::Error::custom)
    // }

    fn deserialize_component_slice<'a, 'de, D: Deserializer<'de>>(
        &self,
        type_id: ComponentTypeId,
        writer: UnknownComponentWriter<'a>,
        deserializer: D,
    ) -> Result<(), D::Error> {
        unimplemented!();
        //let reg = self.comp_types.get(&type_id).unwrap();
        //reg.comp_deserialize(writer.st)


        //if let Some((_, _, _, deserialize, _)) = self.serialize_fns.get(&type_id) {
        //     use serde::de::Error;
        //     let mut deserializer = erased_serde::Deserializer::erase(deserializer);
        //     (deserialize)(storage, &mut deserializer).map_err(D::Error::custom)
        //} else {
            //Err(D::Error::custom("unrecognized component type"))
        //    panic!()
        //}
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

// pub struct CustomDeserializerSeed<'a> {
//     pub deserializer: &'a CustomDeserializer,
// }
//
// impl<'de, 'a> DeserializeSeed<'de> for CustomDeserializerSeed<'a> {
//     type Value = World;
//
//     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
//         where
//             D: serde::Deserializer<'de>,
//     {
//         legion::serialize::DeserializeNewWorld::
//
//         let wrapped = legion::serialize::UniverseDeserializerWrapper(self.deserializer);
//         wrapped.deserialize(deserializer)
//     }
// }
