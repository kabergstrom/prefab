use crate::format::EntityUuid;
use crate::world_serde::{CustomDeserializer /*, CustomDeserializerSeed*/, CustomSerializer};
use legion::World;
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::cell::RefCell;
use std::collections::HashMap;

pub struct CookedPrefab {
    pub world: legion::world::World,
    pub entities: HashMap<EntityUuid, legion::Entity>,
}

impl Serialize for CookedPrefab {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
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

        let mut entity_map =
            HashMap::from_iter(self.entities.iter().map(|(uuid, entity)| (*entity, *uuid)));

        //TODO: Need to use self.entities to serialize
        let custom_serializer = CustomSerializer {
            comp_types: &comp_types,
            entity_map: RefCell::new(&mut entity_map),
        };

        let serializable_world = self
            .world
            .as_serializable(legion::query::any(), &custom_serializer);
        let mut struct_ser = serializer.serialize_struct("CookedPrefab", 2)?;
        struct_ser.serialize_field("entities", &self.entities)?;
        struct_ser.serialize_field("world", &serializable_world)?;
        struct_ser.end()
    }
}

#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum CookedPrefabField {
    Entities,
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
                let entities: HashMap<EntityUuid, legion::Entity> =
                    seq.next_element()?.expect("expected entities");
                let world = seq.next_element::<WorldDeser>()?.expect("expected world");
                Ok(CookedPrefab {
                    world: world.0,
                    entities,
                })
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut entities: Option<HashMap<EntityUuid, legion::Entity>> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        CookedPrefabField::Entities => {
                            entities = Some(map.next_value()?);
                        }
                        CookedPrefabField::World => {
                            let world_deser = map.next_value::<WorldDeser>()?;
                            let entities = entities.expect("expected prefab_meta before world");
                            return Ok(CookedPrefab {
                                world: world_deser.0,
                                entities,
                            });
                        }
                    }
                }
                Err(serde::de::Error::missing_field("data"))
            }
        }
        const FIELDS: &[&str] = &["entities", "world"];
        deserializer.deserialize_struct("Prefab", FIELDS, PrefabDeserVisitor)
    }
}
struct WorldDeser(legion::world::World, HashMap<EntityUuid, legion::Entity>);
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

        let mut entity_map = HashMap::new();
        let mut custom_deserializer = CustomDeserializer {
            comp_types: &comp_types,
            comp_types_uuid: &comp_types_uuid,
            entity_map: RefCell::new(&mut entity_map),
            allocator: RefCell::new(legion::world::Allocate::new()),
        };

        // let custom_deserializer_seed = CustomDeserializerSeed {
        //     deserializer: &custom_deserializer,
        // };

        let seed = legion::serialize::DeserializeNewWorld(&custom_deserializer);

        use serde::de::DeserializeSeed;
        let world: World = seed.deserialize(deserializer).unwrap();

        Ok(WorldDeser(world, entity_map))
    }
}
