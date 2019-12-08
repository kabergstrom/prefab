use crate::format::{ComponentTypeUuid, EntityUuid, PrefabUuid, StorageDeserializer};
use crate::ComponentRegistration;
use serde::Deserializer;
use std::{cell::RefCell, collections::HashMap};


struct ComponentOverride {
    component_type: ComponentTypeUuid,
    data: Vec<u8>,
}
struct PrefabRef {
    overrides: HashMap<EntityUuid, Vec<ComponentOverride>>,
}

struct InnerContext {
    world: legion::world::World,
    registered_components: HashMap<ComponentTypeUuid, ComponentRegistration>,
    prefabs: HashMap<PrefabUuid, Prefab>,
}

struct Context {
    inner: RefCell<InnerContext>,
}

struct Prefab {
    id: PrefabUuid,
    prefab_refs: HashMap<PrefabUuid, PrefabRef>,
    entities: HashMap<EntityUuid, legion::entity::Entity>,
}

impl InnerContext {
    fn get_or_insert_prefab_mut(&mut self, prefab: &PrefabUuid) -> &mut Prefab {
        self.prefabs.entry(*prefab).or_insert_with(|| Prefab {
            id: *prefab,
            entities: HashMap::new(),
            prefab_refs: HashMap::new(),
        })
    }
}

impl StorageDeserializer for &Context {
    fn begin_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid) {
        let mut this = self.inner.borrow_mut();
        let new_entity = this.world.insert((), vec![()])[0];
        let prefab = this.get_or_insert_prefab_mut(prefab);
        prefab.entities.insert(*entity, new_entity);
    }
    fn end_entity_object(&self, prefab: &PrefabUuid, entity: &EntityUuid) {}
    fn deserialize_component<'de, D: Deserializer<'de>>(
        &self,
        prefab: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        let mut this = self.inner.borrow_mut();
        let prefab = this.get_or_insert_prefab_mut(prefab);
        let entity = *prefab
            .entities
            .get(entity)
            // deserializer implementation error, begin_entity_object shall always be called before deserialize_component
            .expect("could not find prefab entity"); 
        let registered = this
            .registered_components
            .get(component_type)
            .ok_or_else(|| <D::Error as serde::de::Error>::custom(format!("Component type {:?} was not registered when deserializing", component_type)))?;
        (registered.deserialize_single_fn)(
            &mut erased_serde::Deserializer::erase(deserializer),
            &mut this.world,
            entity,
        );
        Ok(())
    }
    fn begin_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid) {
        let mut this = self.inner.borrow_mut();
        let prefab = this.get_or_insert_prefab_mut(prefab);
        prefab.prefab_refs.entry(*target_prefab).or_insert_with(|| PrefabRef {
            overrides: HashMap::new(),
        });
    }
    fn end_prefab_ref(&self, prefab: &PrefabUuid, target_prefab: &PrefabUuid) {}
    fn apply_component_diff<'de, D: Deserializer<'de>>(
        &self,
        parent_prefab: &PrefabUuid,
        prefab_ref: &PrefabUuid,
        entity: &EntityUuid,
        component_type: &ComponentTypeUuid,
        deserializer: D,
    ) -> Result<(), D::Error> {
        let mut this = self.inner.borrow_mut();
        let prefab = this.get_or_insert_prefab_mut(parent_prefab);
        let prefab_ref = prefab
            .prefab_refs
            .get_mut(prefab_ref)
            .expect("apply_component_diff called without begin_prefab_ref");
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        serde_transcode::transcode(deserializer, &mut serializer)
            .map_err(<D::Error as serde::de::Error>::custom)?;
        let overrides = prefab_ref
            .overrides
            .entry(*entity)
            .or_insert(Vec::<ComponentOverride>::new());
        overrides.push(ComponentOverride {
            component_type: component_type.clone(),
            data: buffer,
        });
        Ok(())
    }
}
