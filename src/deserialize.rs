use crate::{ComponentTypeUuid, EntityUuid, Prefab, PrefabUuid};
use serde::{
    de::{self, DeserializeSeed, Visitor},
    Deserialize, Deserializer,
};
pub trait Storage {
    /// Called when the deserializer encounters an entity object.
    /// Ideally used to start buffering component data for an entity.
    fn begin_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid);
    /// Called when the deserializer finishes with an entity object.
    /// Ideally finishes buffered storage operations for an entity.
    fn end_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid);
    /// Called when the deserializer encounters component data.
    /// The Storage implementation must handle deserialization of the data,
    /// using the ComponentTypeUuid to identify the type to deserialize as.
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error>;
    /// Called when the deserializer encounters a prefab reference.
    /// The Storage implementation should probably ensure that the referenced prefab
    /// is loaded since this call will most likely be followed by `apply_component_diff` calls.
    /// Alternatively, the implementation can use serde-transcode to save the diff for later.
    fn begin_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid);
    /// Called when the deserializer is finished with a prefab reference.
    fn end_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid);
    /// Called when the deserializer encounters a component diff for a prefab reference.
    /// The Storage implementation must handle deserialization of the diff,
    /// using the ComponentTypeUuid to identify the type to deserialize as.
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        parent_prefab: &PrefabUuid,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error>;
}
struct ComponentOverrideData<'a, S: Storage> {
    pub storage: &'a S,
    pub parent_id: PrefabUuid,
    pub prefab_ref_id: PrefabUuid,
    pub entity_id: EntityUuid,
    pub component_type_id: ComponentTypeUuid,
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for ComponentOverrideData<'a, S> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        <S as Storage>::apply_component_diff(
            self.storage,
            &self.parent_id,
            &self.prefab_ref_id,
            &self.entity_id,
            &self.component_type_id,
            deserializer,
        )
    }
}
struct ComponentOverride<'a, S: Storage> {
    pub storage: &'a S,
    pub parent_id: PrefabUuid,
    pub prefab_ref_id: PrefabUuid,
    pub entity_id: EntityUuid,
}
impl<'a, S: Storage> Clone for ComponentOverride<'a, S> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage,
            parent_id: self.parent_id,
            prefab_ref_id: self.prefab_ref_id,
            entity_id: self.entity_id,
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum ComponentOverrideField {
    ComponentType,
    Diff,
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for ComponentOverride<'a, S> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        impl<'a, 'de, S: Storage> Visitor<'de> for ComponentOverride<'a, S> {
            type Value = ();

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct ComponentOverride")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                println!("visit map component override");
                let mut component_type_id = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        ComponentOverrideField::ComponentType => {
                            if component_type_id.is_some() {
                                return Err(de::Error::duplicate_field("component_type"));
                            }
                            component_type_id = Some(*map.next_value::<uuid::Uuid>()?.as_bytes());
                        }
                        ComponentOverrideField::Diff => {
                            map.next_value_seed(ComponentOverrideData {
                                parent_id: self.parent_id,
                                prefab_ref_id: self.prefab_ref_id,
                                entity_id: self.entity_id,
                                component_type_id: component_type_id.ok_or(
                                    de::Error::missing_field(
                                        "component_type must be serialized before diff",
                                    ),
                                )?,
                                storage: self.storage,
                            })?;
                            return Ok(());
                        }
                    }
                }
                Err(de::Error::missing_field("component_overrides"))
            }
        }
        println!("deserialize component override");
        const FIELDS: &'static [&'static str] = &["component_type", "diff"];
        deserializer.deserialize_struct("ComponentOverride", FIELDS, self)
    }
}
struct EntityOverride<'a, S: Storage> {
    pub storage: &'a S,
    pub parent_id: PrefabUuid,
    pub prefab_ref_id: PrefabUuid,
}
impl<'a, S: Storage> Clone for EntityOverride<'a, S> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage,
            parent_id: self.parent_id,
            prefab_ref_id: self.prefab_ref_id,
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum EntityOverrideField {
    EntityId,
    ComponentOverrides,
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for EntityOverride<'a, S> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        impl<'a, 'de, S: Storage> Visitor<'de> for EntityOverride<'a, S> {
            type Value = ();

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct EntityOverride")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                println!("visit map entity override");
                let mut entity_id = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        EntityOverrideField::EntityId => {
                            if entity_id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            entity_id = Some(*map.next_value::<uuid::Uuid>()?.as_bytes());
                        }
                        EntityOverrideField::ComponentOverrides => {
                            map.next_value_seed(SeqDeserializer(ComponentOverride {
                                parent_id: self.parent_id,
                                prefab_ref_id: self.prefab_ref_id,
                                entity_id: entity_id.ok_or(de::Error::missing_field(
                                    "entity_id must be serialized before component_overrides",
                                ))?,
                                storage: self.storage,
                            }))?;
                            return Ok(());
                        }
                    }
                }
                Err(de::Error::missing_field("component_overrides"))
            }
        }
        println!("deserialize entity override");
        const FIELDS: &'static [&'static str] = &["prefab_id", "component_overrides"];
        deserializer.deserialize_struct("PrefabRef", FIELDS, self)
    }
}
struct PrefabRef<'a, S: Storage> {
    pub storage: &'a S,
    pub parent_id: PrefabUuid,
}
#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum PrefabRefField {
    PrefabId,
    EntityOverrides,
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for PrefabRef<'a, S> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        impl<'a, 'de, S: Storage> Visitor<'de> for PrefabRef<'a, S> {
            type Value = ();

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct PrefabRef")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                println!("visit map prefab ref");
                let mut prefab_id = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        PrefabRefField::PrefabId => {
                            if prefab_id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            prefab_id = Some(*map.next_value::<uuid::Uuid>()?.as_bytes());
                        }
                        PrefabRefField::EntityOverrides => {
                            let prefab_ref_id = prefab_id.ok_or(de::Error::missing_field(
                                "component type must be serialized before data",
                            ))?;
                            self.storage
                                .begin_prefab_ref(&self.parent_id, &prefab_ref_id);
                            map.next_value_seed(SeqDeserializer(EntityOverride {
                                parent_id: self.parent_id,
                                prefab_ref_id,
                                storage: self.storage,
                            }))?;
                            self.storage.end_prefab_ref(&self.parent_id, &prefab_ref_id);
                            return Ok(());
                        }
                    }
                }
                Err(de::Error::missing_field("component_overrides"))
            }
        }
        println!("deserialize prefab ref");
        const FIELDS: &'static [&'static str] = &["prefab_id", "entity_overrides"];
        deserializer.deserialize_struct("PrefabRef", FIELDS, self)
    }
}

struct PrefabObjectDeserializer<'a, S: Storage> {
    pub prefab_id: PrefabUuid,
    pub storage: &'a S,
}
impl<'a, S: Storage> Clone for PrefabObjectDeserializer<'a, S> {
    fn clone(&self) -> Self {
        Self {
            prefab_id: self.prefab_id,
            storage: self.storage,
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "lowercase")]
enum ComponentField {
    Type,
    Data,
}
struct EntityComponentData<'a, S: Storage> {
    prefab_id: PrefabUuid,
    entity_id: EntityUuid,
    component_id: ComponentTypeUuid,
    storage: &'a S,
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for EntityComponentData<'a, S> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        <S as Storage>::deserialize_component(
            self.storage,
            &self.prefab_id,
            &self.entity_id,
            &self.component_id,
            deserializer,
        )
    }
}
struct EntityComponent<'a, S: Storage> {
    prefab_id: PrefabUuid,
    entity_id: EntityUuid,
    storage: &'a S,
}
impl<'a, S: Storage> Clone for EntityComponent<'a, S> {
    fn clone(&self) -> Self {
        Self {
            prefab_id: self.prefab_id,
            entity_id: self.entity_id,
            storage: self.storage,
        }
    }
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for EntityComponent<'a, S> {
    type Value = Prefab;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        impl<'a, 'de, S: Storage> Visitor<'de> for EntityComponent<'a, S> {
            type Value = Prefab;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Entity")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut component_id = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        ComponentField::Type => {
                            if component_id.is_some() {
                                return Err(de::Error::duplicate_field("type"));
                            }
                            component_id = Some(*map.next_value::<uuid::Uuid>()?.as_bytes());
                        }
                        ComponentField::Data => {
                            println!("deserialize component data");
                            map.next_value_seed(EntityComponentData {
                                storage: self.storage,
                                prefab_id: self.prefab_id,
                                entity_id: self.entity_id,
                                component_id: component_id.ok_or(de::Error::missing_field(
                                    "component type must be serialized before data",
                                ))?,
                            })?;
                            return Ok(Prefab {});
                        }
                    }
                }
                Err(de::Error::missing_field("data"))
            }
        }
        println!("deserialize struct");
        const FIELDS: &'static [&'static str] = &["id", "components"];
        deserializer.deserialize_struct("Entity", FIELDS, self)
    }
}

struct EntityPrefabObject<'a, S: Storage>(PrefabObjectDeserializer<'a, S>);
impl<'a, S: Storage> Clone for EntityPrefabObject<'a, S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "lowercase")]
enum EntityPrefabObjectField {
    Id,
    Components,
}
impl<'de, 'a, S: Storage> DeserializeSeed<'de> for EntityPrefabObject<'a, S> {
    type Value = PrefabObjectDeserializer<'a, S>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        impl<'a, 'de, S: Storage> Visitor<'de> for EntityPrefabObject<'a, S> {
            type Value = PrefabObjectDeserializer<'a, S>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Entity")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                println!("visit map entity prefab");
                let mut entity_id = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        EntityPrefabObjectField::Id => {
                            if entity_id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            entity_id = Some(*map.next_value::<uuid::Uuid>()?.as_bytes());
                        }
                        EntityPrefabObjectField::Components => {
                            let entity_id = entity_id.ok_or(de::Error::missing_field(
                                "entity id must be serialized before components",
                            ))?;
                            self.0
                                .storage
                                .begin_entity_object(&self.0.prefab_id, &entity_id);
                            map.next_value_seed(SeqDeserializer(EntityComponent {
                                prefab_id: self.0.prefab_id,
                                entity_id,
                                storage: self.0.storage,
                            }))?;
                            self.0
                                .storage
                                .end_entity_object(&self.0.prefab_id, &entity_id);
                            return Ok(self.0);
                        }
                    }
                }
                Err(de::Error::missing_field("components"))
            }
        }
        println!("deserialize entity prefab");
        const FIELDS: &'static [&'static str] = &["id", "components"];
        deserializer.deserialize_struct("Entity", FIELDS, self)
    }
}

impl<'de, 'a, S: Storage> DeserializeSeed<'de> for PrefabObjectDeserializer<'a, S> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        const VARIANTS: &'static [&'static str] = &["Entity", "PrefabRef"];
        deserializer.deserialize_enum("PrefabObject", VARIANTS, self)
    }
}

impl<'a, 'de, S: Storage> Visitor<'de> for PrefabObjectDeserializer<'a, S> {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("sequence of objects")
    }
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: de::EnumAccess<'de>,
    {
        #[derive(Deserialize, Debug)]
        enum ObjectVariant {
            Entity,
            PrefabRef,
        }
        match de::EnumAccess::variant(data)? {
            (ObjectVariant::Entity, variant) => {
                de::VariantAccess::newtype_variant_seed::<EntityPrefabObject<S>>(
                    variant,
                    EntityPrefabObject(self),
                )?;
                Ok(())
            }
            (ObjectVariant::PrefabRef, variant) => {
                println!("got prefab ref ");
                de::VariantAccess::newtype_variant_seed::<PrefabRef<S>>(
                    variant,
                    PrefabRef {
                        parent_id: self.prefab_id,
                        storage: self.storage,
                    },
                )?;
                Ok(())
            }
        }
    }
}
pub struct SeqDeserializer<T>(T);

impl<'de, T: DeserializeSeed<'de> + Clone> DeserializeSeed<'de> for SeqDeserializer<T> {
    type Value = Prefab;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}
impl<'de, T: DeserializeSeed<'de> + Clone> Visitor<'de> for SeqDeserializer<T> {
    type Value = Prefab;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("sequence of objects")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        while let Some(_) = seq.next_element_seed::<T>(self.0.clone())? {}
        Ok(Prefab {})
    }
}

pub struct PrefabDeserializer<'a, S: Storage> {
    pub storage: &'a S,
}
impl<'de, 'a: 'de, S: Storage> DeserializeSeed<'de> for PrefabDeserializer<'a, S> {
    type Value = Prefab;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &'static [&'static str] = &["id", "objects"];
        deserializer.deserialize_struct("Prefab", FIELDS, self)
    }
}

#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "lowercase")]
enum PrefabField {
    Id,
    Objects,
}
impl<'a: 'de, 'de, S: Storage> Visitor<'de> for PrefabDeserializer<'a, S> {
    type Value = Prefab;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct Prefab")
    }

    fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
    where
        V: de::MapAccess<'de>,
    {
        println!("visit map");
        let mut prefab_id = None;
        let mut prefab = None;
        while let Some(key) = map.next_key()? {
            match key {
                PrefabField::Id => {
                    if prefab_id.is_some() {
                        return Err(de::Error::duplicate_field("id"));
                    }
                    prefab_id = Some(*map.next_value::<uuid::Uuid>()?.as_bytes());
                    println!("prefab id {:?}", prefab_id);
                }
                PrefabField::Objects => {
                    prefab = Some(map.next_value_seed(SeqDeserializer(
                        PrefabObjectDeserializer {
                            prefab_id: prefab_id.ok_or(de::Error::missing_field(
                                "prefab ID must be serialized before prefab objects",
                            ))?,
                            storage: self.storage,
                        },
                    ))?);
                }
            }
        }
        prefab.ok_or(de::Error::missing_field("objects"))
    }
}
