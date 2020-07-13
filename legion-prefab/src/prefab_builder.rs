use legion::*;
use prefab_format::{EntityUuid, ComponentTypeUuid, PrefabUuid};

use std::collections::HashMap;
use std::collections::HashSet;
use crate::{ComponentRegistration, DiffSingleResult, ComponentOverride, PrefabMeta, PrefabRef};
use crate::{CookedPrefab, CopyCloneImpl, Prefab};
use fnv::FnvHashMap;
use std::hash::BuildHasher;

pub struct PrefabBuilder {
    // This is the snapshot of the world when the transaction starts
    before_world: legion::world::World,

    // This is the world that a downstream caller can manipulate. We will diff the data here against
    // the before_world to produce diffs
    after_world: legion::world::World,

    parent_prefab: PrefabUuid,
}

#[derive(Debug)]
pub enum PrefabBuilderError {
    EntityDeleted,
    ComponentRemoved,
    ComponentAdded,
}

impl PrefabBuilder {
    pub fn new<S: BuildHasher>(
        prefab_uuid: PrefabUuid,
        prefab: CookedPrefab,
        universe: &Universe,
        mut clone_impl: CopyCloneImpl<S>,
    ) -> Self {
        let mut before_world = universe.create_world();
        before_world.clone_from(
            &prefab.world,
            &legion::query::any(),
            &mut clone_impl,
        );

        let mut after_world = universe.create_world();
        after_world.clone_from(
            &prefab.world,
            &legion::query::any(),
            &mut clone_impl,
        );

        PrefabBuilder {
            before_world,
            after_world,
            parent_prefab: prefab_uuid,
        }
    }

    pub fn world(&self) -> &World {
        &self.after_world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.after_world
    }

    pub fn create_prefab<S: BuildHasher>(
        &mut self,
        universe: &Universe,
        registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration>,
        mut clone_impl: CopyCloneImpl<S>,
    ) -> Result<Prefab, PrefabBuilderError> {
        let mut new_prefab_world = universe.create_world();

        // Find all the entities in the before world. Check that they weren't deleted (which we
        // don't support).
        let mut all = Entity::query();
        for before_entity in all.iter(&self.before_world) {
            if !self.after_world.contains(*before_entity) {
                // We do not support deleting entities in child prefabs
                return Err(PrefabBuilderError::EntityDeleted);
            }
        }

        // Find the entities that have been added (i.e. are in the after_world but not the
        // before_world) and copy them into new_prefab_world
        for after_entity in all.iter(&self.after_world) {
            if !self.before_world.contains(*after_entity) {
                new_prefab_world.clone_from_single(
                    &self.after_world,
                    *after_entity,
                    &mut clone_impl,
                );
            }
        }

        let mut entity_overrides = HashMap::new();
        for entity in all.iter(&self.after_world) {
            let mut component_overrides = vec![];

            for (component_type, registration) in registered_components {
                let mut ron_ser = ron::ser::Serializer::new(None, true);
                let mut erased = erased_serde::Serializer::erase(&mut ron_ser);

                let result = registration.diff_single(
                    &mut erased,
                    &self.before_world,
                    Some(*entity),
                    &self.after_world,
                    Some(*entity),
                );

                match result {
                    DiffSingleResult::NoChange => {
                        // Do nothing
                    }
                    DiffSingleResult::Change => {
                        // Store the change
                        component_overrides.push(ComponentOverride {
                            component_type: *component_type,
                            data: ron_ser.into_output_string(),
                        })
                    }
                    DiffSingleResult::Add => {
                        // Fail, a component was added. This is not supported
                        return Err(PrefabBuilderError::ComponentAdded);
                    }
                    DiffSingleResult::Remove => {
                        // Fail, a component was deleted. This is not supported
                        return Err(PrefabBuilderError::ComponentRemoved);
                    }
                }
            }
            let entity_uuid = universe.canon().get_name(*entity).unwrap();
            if !component_overrides.is_empty() {
                entity_overrides.insert(entity_uuid, component_overrides);
            }
        }

        let prefab_ref = PrefabRef {
            overrides: entity_overrides,
        };

        let mut prefab_refs = HashMap::new();
        prefab_refs.insert(self.parent_prefab, prefab_ref);

        let prefab_meta = PrefabMeta {
            id: *uuid::Uuid::new_v4().as_bytes(),
            prefab_refs,
        };

        Ok(Prefab {
            world: new_prefab_world,
            prefab_meta,
        })
    }
}
