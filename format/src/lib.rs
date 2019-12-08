use serde::Deserializer;
mod deserialize;
pub use deserialize::Storage as StorageDeserializer;
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