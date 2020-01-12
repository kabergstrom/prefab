use serde::{Serializer, Deserializer};
mod deserialize;
mod serialize;
pub use deserialize::Storage as StorageDeserializer;
pub use serialize::StorageSerializer;
pub type PrefabUuid = uuid::Bytes;
pub type EntityUuid = uuid::Bytes;
pub type ComponentTypeUuid = type_uuid::Bytes;
pub fn deserialize<'de, 'a: 'de, D: Deserializer<'de>, S: StorageDeserializer>(
    deserializer: D,
    storage: &'a S,
) -> Result<(), D::Error> {
    let prefab_deserializer = crate::deserialize::PrefabDeserializer { storage };
    <deserialize::PrefabDeserializer<'a, S> as serde::de::DeserializeSeed>::deserialize(
        prefab_deserializer,
        deserializer,
    )
}

pub fn serialize<'a, S: Serializer, SS: StorageSerializer>(
    serializer: S,
    storage: &'a SS,
    prefab_id: PrefabUuid,
) -> Result<S::Ok, S::Error> {
    let prefab_serializer = crate::serialize::PrefabSerializer::new(prefab_id, storage);
    <serialize::PrefabSerializer<'a, SS> as serde::ser::Serialize>::serialize(
        &prefab_serializer,
        serializer,
    )
}