use crate::format::EntityUuid;
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::collections::HashMap;

pub struct CookedPrefab {
    pub world: legion::world::World,
    pub entities: HashMap<EntityUuid, legion::entity::Entity>,
}

impl Serialize for CookedPrefab {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        use std::iter::FromIterator;
        let tag_types = HashMap::from_iter(
            crate::registration::iter_tag_registrations()
                .map(|reg| (legion::storage::TagTypeId(reg.ty()), reg.clone())),
        );
        let comp_types = HashMap::from_iter(
            crate::registration::iter_component_registrations()
                .map(|reg| (legion::storage::ComponentTypeId(reg.ty()), reg.clone())),
        );

        let mut entity_map = HashMap::with_capacity(self.entities.len());
        for (k, v) in &self.entities {
            entity_map.insert(*v, *k);
        }

        let serialize_impl = crate::SerializeImpl::new(tag_types, comp_types, entity_map);
        let serializable_world = legion::ser::serializable_world(&self.world, &serialize_impl);
        let mut struct_ser = serializer.serialize_struct("CookedPrefab", 2)?;
        struct_ser.serialize_field("world", &serializable_world)?;
        struct_ser.end()
    }
}

#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum PrefabField {
    World,
}
impl<'de> Deserialize<'de> for CookedPrefab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrefabDeserVisitor;
        impl<'de> serde::de::Visitor<'de> for PrefabDeserVisitor {
            type Value = CookedPrefab;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct CookedPrefab")
            }
            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let world = seq.next_element::<WorldDeser>()?.expect("expected world");
                Ok(CookedPrefab {
                    world: world.0,
                    entities: world.1,
                })
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                if let Some(key) = map.next_key()? {
                    match key {
                        PrefabField::World => {
                            let world_deser = map.next_value::<WorldDeser>()?;
                            return Ok(CookedPrefab {
                                world: world_deser.0,
                                entities: world_deser.1,
                            });
                        }
                    }
                }
                Err(serde::de::Error::missing_field("data"))
            }
        }
        const FIELDS: &[&str] = &["prefab_meta", "world"];
        deserializer.deserialize_struct("Prefab", FIELDS, PrefabDeserVisitor)
    }
}
struct WorldDeser(
    legion::world::World,
    HashMap<uuid::Bytes, legion::entity::Entity>,
);
impl<'de> Deserialize<'de> for WorldDeser {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::iter::FromIterator;
        let tag_types = HashMap::from_iter(
            crate::registration::iter_tag_registrations()
                .map(|reg| (legion::storage::TagTypeId(reg.ty()), reg.clone())),
        );
        let comp_types = HashMap::from_iter(
            crate::registration::iter_component_registrations()
                .map(|reg| (legion::storage::ComponentTypeId(reg.ty()), reg.clone())),
        );
        let deserialize_impl = crate::DeserializeImpl::new(tag_types, comp_types);

        // TODO support sharing universe
        let mut world = legion::world::World::new();
        let deserializable_world = legion::de::deserializable(&mut world, &deserialize_impl);
        serde::de::DeserializeSeed::deserialize(deserializable_world, deserializer)?;
        Ok(WorldDeser(world, deserialize_impl.entity_map.into_inner()))
    }
}
