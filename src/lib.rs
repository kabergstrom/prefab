use serde::{
    de::{self, DeserializeSeed, Visitor},
    ser, Deserialize, Deserializer, Serialize,
};
use type_uuid::TypeUuid;
mod deserialize;
pub type PrefabUuid = uuid::Bytes;
pub type EntityUuid = uuid::Bytes;
pub type ComponentTypeUuid = type_uuid::Bytes;
pub struct Prefab {}
impl Prefab {
    pub fn deserialize<'de, 'a: 'de, D: Deserializer<'de>, S: Storage>(
        deserializer: D,
        storage: &'a S,
    ) -> Result<Prefab, D::Error> {
        let prefab_deserializer = crate::deserialize::PrefabDeserializer { storage };
        <deserialize::PrefabDeserializer<'a, S> as serde::de::DeserializeSeed>::deserialize(
            prefab_deserializer,
            deserializer,
        )
    }
}

pub trait Storage {
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error>;
    fn add_prefab_ref<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        target_prefab: &PrefabUuid,
    );
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        parent_prefab: &PrefabUuid,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error>;
}
