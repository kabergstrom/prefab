use crate::{PrefabUuid, EntityUuid, ComponentTypeUuid};
use serde::{
    Serialize, Serializer,
    ser::{SerializeSeq, SerializeStruct},
};

pub struct PrefabSerializer<'a, SS: StorageSerializer> {
    storage: &'a SS,
    prefab_id: PrefabUuid,
}
impl<'a, SS: StorageSerializer> PrefabSerializer<'a, SS> {
    pub fn new(
        prefab_id: PrefabUuid,
        storage: &'a SS,
    ) -> Self {
        Self { storage, prefab_id }
    }
}
pub trait StorageSerializer {
    fn entities(&self) -> Vec<EntityUuid>;
    fn component_types(
        &self,
        entity: &EntityUuid,
    ) -> Vec<ComponentTypeUuid>;
    fn serialize_entity_component<S: Serializer>(
        &self,
        serializer: S,
        entity: &EntityUuid,
        component: &ComponentTypeUuid,
    ) -> Result<S::Ok, S::Error>;
    fn prefab_refs(&self) -> Vec<PrefabUuid>;
    fn prefab_ref_overrides(
        &self,
        uuid: &PrefabUuid,
    ) -> Vec<(EntityUuid, Vec<ComponentTypeUuid>)>;
    fn serialize_component_override_diff<S: Serializer>(
        &self,
        serializer: S,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component: &ComponentTypeUuid,
    ) -> Result<S::Ok, S::Error>;
}

#[derive(Serialize)]
struct PrefabEntity<'a, SS: StorageSerializer> {
    id: uuid::Uuid,
    #[serde(bound(serialize = "SS: StorageSerializer"))]
    components: &'a [EntityComponent<'a, SS>],
}
#[derive(Serialize)]
struct EntityComponent<'a, SS: StorageSerializer> {
    r#type: uuid::Uuid,
    #[serde(bound(serialize = "SS: StorageSerializer"))]
    data: EntityComponentSerializer<'a, SS>,
}

struct EntityComponentSerializer<'a, SS: StorageSerializer> {
    storage: &'a SS,
    id: EntityUuid,
    component: ComponentTypeUuid,
}

struct EntityPrefabObjectSerializer<'a, SS: StorageSerializer> {
    storage: &'a SS,
    id: EntityUuid,
}

struct ComponentOverrideDiff<'a, SS: StorageSerializer> {
    storage: &'a SS,
    prefab_ref: PrefabUuid,
    entity: EntityUuid,
    component_type: ComponentTypeUuid,
}
#[derive(Serialize)]
struct ComponentOverride<'a, SS: StorageSerializer> {
    component_type: uuid::Uuid,
    #[serde(bound(serialize = "SS: StorageSerializer"))]
    diff: ComponentOverrideDiff<'a, SS>,
}
#[derive(Serialize)]
struct EntityOverride<'a, SS: StorageSerializer> {
    entity_id: uuid::Uuid,
    #[serde(bound(serialize = "SS: StorageSerializer"))]
    component_overrides: Vec<ComponentOverride<'a, SS>>,
}

#[derive(Serialize)]
struct PrefabRef<'a, SS: StorageSerializer> {
    prefab_id: uuid::Uuid,
    #[serde(bound(serialize = "SS: StorageSerializer"))]
    entity_overrides: &'a [EntityOverride<'a, SS>],
}
struct PrefabRefObjectSerializer<'a, SS: StorageSerializer> {
    storage: &'a SS,
    id: PrefabUuid,
}
struct ObjectArraySerializer<'a, SS: StorageSerializer> {
    storage: &'a SS,
}

impl<'a, SS: StorageSerializer> Serialize for EntityComponentSerializer<'a, SS> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.storage
            .serialize_entity_component(serializer, &self.id, &self.component)
    }
}

impl<'a, SS: StorageSerializer> Serialize for EntityPrefabObjectSerializer<'a, SS> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_variant(
            "PrefabObject",
            0,
            "Entity",
            &PrefabEntity {
                id: uuid::Uuid::from_bytes(self.id),
                components: &self
                    .storage
                    .component_types(&self.id)
                    .iter()
                    .map(|c| EntityComponent {
                        r#type: uuid::Uuid::from_bytes(*c),
                        data: EntityComponentSerializer {
                            storage: self.storage,
                            id: self.id,
                            component: *c,
                        },
                    })
                    .collect::<Vec<_>>(),
            },
        )
    }
}

impl<'a, SS: StorageSerializer> Serialize for ComponentOverrideDiff<'a, SS> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.storage.serialize_component_override_diff(
            serializer,
            &self.prefab_ref,
            &self.entity,
            &self.component_type,
        )
    }
}

impl<'a, SS: StorageSerializer> Serialize for PrefabRefObjectSerializer<'a, SS> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_variant(
            "PrefabObject",
            0,
            "PrefabRef",
            &PrefabRef {
                prefab_id: uuid::Uuid::from_bytes(self.id),
                entity_overrides: &self
                    .storage
                    .prefab_ref_overrides(&self.id)
                    .iter()
                    .map(|(entity, component_types)| EntityOverride {
                        entity_id: uuid::Uuid::from_bytes(*entity),
                        component_overrides: component_types
                            .iter()
                            .map(|component_type| ComponentOverride {
                                component_type: uuid::Uuid::from_bytes(*component_type),
                                diff: ComponentOverrideDiff {
                                    storage: self.storage,
                                    prefab_ref: self.id,
                                    entity: *entity,
                                    component_type: *component_type,
                                },
                            })
                            .collect::<Vec<_>>(),
                    })
                    .collect::<Vec<_>>(),
            },
        )
    }
}

impl<'a, SS: StorageSerializer> Serialize for ObjectArraySerializer<'a, SS> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let entities = self.storage.entities();
        let prefab_refs = self.storage.prefab_refs();
        let mut seq = serializer.serialize_seq(Some(entities.len() + prefab_refs.len()))?;
        for s in prefab_refs
            .iter()
            .map(|prefab_ref| PrefabRefObjectSerializer {
                storage: self.storage,
                id: *prefab_ref,
            })
        {
            seq.serialize_element(&s)?;
        }
        for s in entities.iter().map(|entity| EntityPrefabObjectSerializer {
            storage: self.storage,
            id: *entity,
        }) {
            seq.serialize_element(&s)?;
        }

        seq.end()
    }
}

impl<'a, SS: StorageSerializer> Serialize for PrefabSerializer<'a, SS> {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("Prefab", 2)?;
        s.serialize_field("id", &uuid::Uuid::from_bytes(self.prefab_id))?;
        s.serialize_field(
            "objects",
            &ObjectArraySerializer {
                storage: self.storage,
            },
        )?;
        s.end()
    }
}
