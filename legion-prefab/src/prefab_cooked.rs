use crate::format::EntityUuid;
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::collections::HashMap;
use crate::world_serde::{CustomSerializer, CustomDeserializer, CustomDeserializerSeed};
use legion::World;

pub struct CookedPrefab {
    pub world: legion::world::World,
}

impl Serialize for CookedPrefab {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        use std::iter::FromIterator;

        //TODO: Not good to allocate and throw away
        let comp_types = HashMap::from_iter(
            crate::registration::iter_component_registrations()
                .map(|reg| (reg.component_type_id(), reg.clone())),
        );

        let custom_serializer = CustomSerializer {
            comp_types,
        };

        let serializable_world = self.world.as_serializable(legion::query::any(), &custom_serializer);
        let mut struct_ser = serializer.serialize_struct("CookedPrefab", 1)?;
        struct_ser.serialize_field("world", &serializable_world)?;
        struct_ser.end()
    }
}

#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum CookedPrefabField {
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

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("struct CookedPrefab")
            }
            fn visit_seq<V>(
                self,
                mut seq: V,
            ) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let world = seq.next_element::<WorldDeser>()?.expect("expected world");
                Ok(CookedPrefab {
                    world: world.0,
                })
            }

            fn visit_map<V>(
                self,
                mut map: V,
            ) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                while let Some(key) = map.next_key()? {
                    match key {
                        CookedPrefabField::World => {
                            let world_deser = map.next_value::<WorldDeser>()?;
                            return Ok(CookedPrefab {
                                world: world_deser.0,
                            });
                        }
                    }
                }
                Err(serde::de::Error::missing_field("data"))
            }
        }
        const FIELDS: &[&str] = &["world"];
        deserializer.deserialize_struct("Prefab", FIELDS, PrefabDeserVisitor)
    }
}
struct WorldDeser(
    legion::world::World,
);
impl<'de> Deserialize<'de> for WorldDeser {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use std::iter::FromIterator;

        //TODO: Not good to allocate and throw away
        let comp_types = HashMap::from_iter(
            crate::registration::iter_component_registrations()
                .map(|reg| (reg.component_type_id(), reg.clone())),
        );
        let comp_types_uuid = HashMap::from_iter(
            crate::registration::iter_component_registrations()
                .map(|reg| (*reg.uuid(), reg.clone())),
        );

        let custom_deserializer = CustomDeserializer {
            comp_types,
            comp_types_uuid
        };
        // TODO support sharing universe
        let universe = legion::Universe::new();
        let custom_deserializer_seed = CustomDeserializerSeed {
            deserializer: &custom_deserializer,
            universe: &universe
        };
        use serde::de::DeserializeSeed;
        let world: World = custom_deserializer_seed.deserialize(deserializer).unwrap();
        Ok(WorldDeser(world))
    }
}
