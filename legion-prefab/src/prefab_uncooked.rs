use crate::format::{ComponentTypeUuid, EntityUuid, PrefabUuid, StorageDeserializer, StorageSerializer};
use crate::world_serde::{CustomDeserializer, CustomSerializer};
use crate::ComponentRegistration;
use legion::storage::ComponentTypeId;
use legion::*;
use serde::de::DeserializeSeed;
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::hash::BuildHasher;
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
};

/// The data we override on a component of an entity in another prefab that we reference
#[derive(Serialize, Deserialize)]
pub struct ComponentOverride {
    /// The component type to which we will apply this override data
    pub component_type: ComponentTypeUuid,

    /// The data used to override (in Ron-encoded serde_diff format)
    pub data: String,
}

/// Represents a reference from one prefab to another, along with the data with which it should be
/// overridden
#[derive(Serialize, Deserialize)]
pub struct PrefabRef {
    /// The entities in the other prefab we will override and the data with which to override them
    pub overrides: HashMap<EntityUuid, Vec<ComponentOverride>>,
}

#[derive(Serialize, Deserialize)]
/// Represents a list of entities in this prefab and references to other prefabs
pub struct PrefabMeta {
    /// Unique ID of this prefab
    pub id: PrefabUuid,

    /// The other prefabs that this prefab will include, plus the data we will override them with
    pub prefab_refs: HashMap<PrefabUuid, PrefabRef>,

    #[serde(skip, default)]
    // The entities that are stored in this prefab
    pub entities: HashMap<EntityUuid, Entity>,
}

/// The uncooked prefab format. Raw entity data is stored in the legion::World. Metadata includes
/// component overrides and mappings from EntityUuid to legion::Entity
pub struct Prefab {
    /// The legion world contains entity data for all entities in this prefab. (EntityRef data is
    /// not included)
    pub world: World,

    /// Metadata for the prefab (references to other prefabs and mappings of EntityUUID to
    /// Entity
    pub prefab_meta: PrefabMeta,
}

impl Prefab {
    pub fn new(world: World) -> Self {
        let mut entities = HashMap::new();

        let mut all = Entity::query();
        for entity in all.iter(&world) {
            entities.insert(*uuid::Uuid::new_v4().as_bytes(), *entity);
        }

        let prefab_meta = PrefabMeta {
            id: *uuid::Uuid::new_v4().as_bytes(),
            entities,
            prefab_refs: Default::default(),
        };

        Prefab { world, prefab_meta }
    }

    pub fn prefab_id(&self) -> PrefabUuid {
        self.prefab_meta.id
    }
}

pub struct PrefabSerdeContext<'a, T: BuildHasher> {
    pub registered_components: &'a HashMap<ComponentTypeUuid, ComponentRegistration, T>,
}

// Manual impl because T is not Clone
impl<'a, T: BuildHasher> Clone for PrefabSerdeContext<'a, T> {
    fn clone(&self) -> Self {
        PrefabSerdeContext {
            registered_components: self.registered_components,
        }
    }
}

// Manual impl because T is not Copy
impl<'a, T: BuildHasher> Copy for PrefabSerdeContext<'a, T> {}

pub struct PrefabFormatDeserializer<'a, T: BuildHasher> {
    prefab: RefCell<Option<Prefab>>,
    context: PrefabSerdeContext<'a, T>,
}
impl<'a, T: BuildHasher> PrefabFormatDeserializer<'a, T> {
    pub fn new(context: PrefabSerdeContext<'a, T>) -> Self {
        Self {
            prefab: RefCell::new(None),
            context,
        }
    }
    pub fn prefab(self) -> Prefab {
        self.prefab
            .into_inner()
            .expect("no valid prefab - make sure to deserialize before calling prefab()")
    }
}

impl<'a, T: BuildHasher> PrefabFormatDeserializer<'a, T> {
    fn get_or_insert_prefab_mut(
        &self,
        prefab_uuid: &PrefabUuid,
    ) -> RefMut<Prefab> {
        let mut prefab_cell = self.prefab.borrow_mut();
        if let Some(prefab) = &*prefab_cell {
            assert!(prefab.prefab_meta.id == *prefab_uuid);
        } else {
            prefab_cell.replace(Prefab {
                // TODO support sharing universe
                world: World::default(),
                prefab_meta: PrefabMeta {
                    id: *prefab_uuid,
                    entities: HashMap::new(),
                    prefab_refs: HashMap::new(),
                },
            });
        }

        RefMut::map(prefab_cell, |opt| opt.as_mut().unwrap())
    }
}

// This implementation takes care of reading a prefab source file. As we walk through the source
// file the functions here are called and we build out the data
impl<T: BuildHasher> StorageDeserializer for PrefabFormatDeserializer<'_, T> {
    fn begin_prefab(
        &self,
        prefab: &PrefabUuid,
    ) {
        self.get_or_insert_prefab_mut(prefab);
    }
    fn begin_entity_object(
        &self,
        prefab: &PrefabUuid,
        entity: &EntityUuid,
    ) {
        let mut prefab = self.get_or_insert_prefab_mut(prefab);
        let new_entity = prefab.world.push(());
        prefab.prefab_meta.entities.insert(*entity, new_entity);
    }
    fn end_entity_object(
        &self,
        _prefab: &PrefabUuid,
        _entity: &EntityUuid,
    ) {
    }
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        let mut prefab = self.get_or_insert_prefab_mut(prefab);
        let entity = *prefab
            .prefab_meta
            .entities
            .get(entity)
            // deserializer implementation error, begin_entity_object shall always be called before deserialize_component
            .expect("could not find prefab entity");

        let registered = self
            .context
            .registered_components
            .get(component_type)
            .ok_or_else(|| {
                <D::Error as serde::de::Error>::custom(format!(
                    "Component type {:?} was not registered when deserializing",
                    component_type
                ))
            })?;

        registered.add_to_entity(
            &mut erased_serde::Deserializer::erase(deserializer),
            &mut prefab.world,
            entity,
        );
        Ok(())
    }
    fn begin_prefab_ref(
        &self,
        prefab: &PrefabUuid,
        target_prefab: &PrefabUuid,
    ) {
        let mut prefab = self.get_or_insert_prefab_mut(prefab);
        prefab
            .prefab_meta
            .prefab_refs
            .entry(*target_prefab)
            .or_insert_with(|| PrefabRef {
                overrides: HashMap::new(),
            });
    }
    fn end_prefab_ref(
        &self,
        _prefab: &PrefabUuid,
        _target_prefab: &PrefabUuid,
    ) {
    }
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        parent_prefab: &PrefabUuid,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        let mut prefab = self.get_or_insert_prefab_mut(parent_prefab);
        let prefab_ref = prefab
            .prefab_meta
            .prefab_refs
            .get_mut(prefab_ref)
            .expect("apply_component_diff called without begin_prefab_ref");
        let overrides = prefab_ref
            .overrides
            .entry(*entity)
            .or_insert_with(Vec::<ComponentOverride>::new);
        overrides.push(ComponentOverride {
            component_type: *component_type,
            data: String::deserialize(deserializer)?,
        });
        Ok(())
    }
}

impl Serialize for Prefab {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        use std::iter::FromIterator;

        //TODO: Not good to allocate and throw away
        let comp_types = HashMap::from_iter(
            crate::registration::iter_component_registrations()
                .map(|reg| (reg.component_type_id(), reg.clone())),
        );
        let mut entity_map = HashMap::from_iter(
            self.prefab_meta
                .entities
                .iter()
                .map(|(uuid, entity)| (*entity, *uuid)),
        );

        let custom_serializer = CustomSerializer {
            comp_types: &comp_types,
            entity_map: RefCell::new(&mut entity_map),
        };

        let serializable_world = self
            .world
            .as_serializable(legion::query::any(), &custom_serializer);
        let mut struct_ser = serializer.serialize_struct("Prefab", 2)?;
        struct_ser.serialize_field("prefab_meta", &self.prefab_meta)?;
        struct_ser.serialize_field("world", &serializable_world)?;
        struct_ser.end()
    }
}

#[derive(Deserialize, Debug)]
#[serde(field_identifier, rename_all = "snake_case")]
enum PrefabField {
    PrefabMeta,
    World,
}
impl<'de> Deserialize<'de> for Prefab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrefabDeserVisitor;
        impl<'de> serde::de::Visitor<'de> for PrefabDeserVisitor {
            type Value = Prefab;

            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("struct Prefab")
            }
            fn visit_seq<V>(
                self,
                mut seq: V,
            ) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let mut prefab_meta: PrefabMeta =
                    seq.next_element()?.expect("expected prefab_meta");
                let world = seq.next_element::<WorldDeser>()?.expect("expected world");
                prefab_meta.entities = world.1;
                Ok(Prefab {
                    prefab_meta,
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
                let mut prefab_meta: Option<PrefabMeta> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        PrefabField::PrefabMeta => {
                            prefab_meta = Some(map.next_value()?);
                        }
                        PrefabField::World => {
                            let world_deser = map.next_value::<WorldDeser>()?;
                            let mut prefab_meta =
                                prefab_meta.expect("expected prefab_meta before world");
                            prefab_meta.entities = world_deser.1;
                            return Ok(Prefab {
                                prefab_meta,
                                world: world_deser.0,
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
struct WorldDeser(World, HashMap<EntityUuid, legion::Entity>);
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
        let custom_deserializer = CustomDeserializer {
            comp_types: &comp_types,
            comp_types_uuid: &comp_types_uuid,
            entity_map: RefCell::new(&mut entity_map),
            allocator: RefCell::new(legion::world::Allocate::new()),
        };

        let seed = legion::serialize::DeserializeNewWorld(&custom_deserializer);
        let world: World = seed.deserialize(deserializer).unwrap();

        Ok(WorldDeser(world, entity_map))
    }
}

pub struct PrefabFormatSerializer<'a, 'b, T: BuildHasher> {
    prefab: &'b Prefab,
    context: PrefabSerdeContext<'a, T>,
    type_id_to_uuid: HashMap<ComponentTypeId, ComponentTypeUuid>,
}
impl<'a, 'b, T: BuildHasher> PrefabFormatSerializer<'a, 'b, T> {
    pub fn new(
        context: PrefabSerdeContext<'a, T>,
        prefab: &'b Prefab,
    ) -> Self {
        use std::iter::FromIterator;
        Self {
            prefab,
            context,
            type_id_to_uuid: HashMap::from_iter(
                context
                    .registered_components
                    .iter()
                    .map(|(type_id, reg)| (reg.component_type_id(), *type_id)),
            ),
        }
    }
}
impl<T: BuildHasher> StorageSerializer for PrefabFormatSerializer<'_, '_, T> {
    fn entities(&self) -> Vec<EntityUuid> {
        self.prefab.prefab_meta.entities.keys().cloned().collect()
    }

    fn component_types(
        &self,
        entity_uuid: &EntityUuid,
    ) -> Vec<ComponentTypeUuid> {
        let entity = self.prefab.prefab_meta.entities[entity_uuid];
        let e = self
            .prefab
            .world
            .entry_ref(entity)
            .expect("entity not in World when serializing prefab");

        e.archetype()
            .layout()
            .component_types()
            .iter()
            .filter_map(|type_id| self.type_id_to_uuid.get(type_id).cloned())
            .filter(|type_id| self.context.registered_components.contains_key(type_id))
            .collect()
    }
    fn serialize_entity_component<S: Serializer>(
        &self,
        serializer: S,
        entity_uuid: &EntityUuid,
        component: &ComponentTypeUuid,
    ) -> Result<S::Ok, S::Error> {
        let mut result = None;
        let mut serializer = Some(serializer);
        let entity = self.prefab.prefab_meta.entities[entity_uuid];
        self.context.registered_components[component].serialize_single(
            &self.prefab.world,
            entity,
            &mut |comp| {
                result = Some(erased_serde::serialize(comp, serializer.take().unwrap()));
            },
        );
        result.unwrap()
    }
    fn prefab_refs(&self) -> Vec<PrefabUuid> {
        self.prefab
            .prefab_meta
            .prefab_refs
            .keys()
            .cloned()
            .collect()
    }
    fn prefab_ref_overrides(
        &self,
        uuid: &PrefabUuid,
    ) -> Vec<(EntityUuid, Vec<ComponentTypeUuid>)> {
        let prefab_ref = &self.prefab.prefab_meta.prefab_refs[uuid];
        prefab_ref
            .overrides
            .iter()
            .map(|(entity_uuid, comps)| {
                (
                    *entity_uuid,
                    comps.iter().map(|comp| comp.component_type).collect(),
                )
            })
            .collect()
    }
    fn serialize_component_override_diff<S: Serializer>(
        &self,
        serializer: S,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component: &ComponentTypeUuid,
    ) -> Result<S::Ok, S::Error> {
        let prefab_ref = &self.prefab.prefab_meta.prefab_refs[prefab_ref];
        let comp_override = prefab_ref.overrides[entity]
            .iter()
            .find(|o| &o.component_type == component)
            .expect("invalid component type when serializing component override diff");
        comp_override.data.serialize(serializer)
    }
}
