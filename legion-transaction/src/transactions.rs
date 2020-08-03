use legion::*;
use prefab_format::{EntityUuid, ComponentTypeUuid};

use std::collections::HashMap;
use std::collections::HashSet;
use legion_prefab::{ComponentRegistration, DiffSingleResult};
use crate::component_diffs::{ComponentDiff, EntityDiff, EntityDiffOp, WorldDiff};
use legion_prefab::CopyCloneImpl;
use std::hash::BuildHasher;

struct TransactionBuilderEntityInfo {
    entity_uuid: EntityUuid,
    entity: Entity,
}

#[derive(Default)]
pub struct TransactionBuilder {
    entities: Vec<TransactionBuilderEntityInfo>,
}

impl TransactionBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(
        mut self,
        entity: Entity,
        entity_uuid: EntityUuid,
    ) -> Self {
        self.entities.push(TransactionBuilderEntityInfo {
            entity,
            entity_uuid,
        });
        self
    }

    pub fn begin<S: BuildHasher>(
        self,
        universe: &Universe,
        src_world: &World,
        mut clone_impl: CopyCloneImpl<S>,
    ) -> Transaction {
        let mut before_world = universe.create_world();
        let mut after_world = universe.create_world();

        for entity_info in self.entities {
            //TODO: Propagate error
            before_world.clone_from_single(&src_world, entity_info.entity, &mut clone_impl).unwrap();
            after_world.clone_from_single(&src_world, entity_info.entity, &mut clone_impl).unwrap();
        }

        Transaction {
            before_world,
            after_world,
        }
    }
}

//TODO: Remove this if possible
pub struct TransactionEntityInfo {
    before_entity: Option<Entity>,
    after_entity: Option<Entity>,
}

impl TransactionEntityInfo {
    pub fn new(
        before_entity: Option<Entity>,
        after_entity: Option<Entity>,
    ) -> Self {
        TransactionEntityInfo {
            before_entity,
            after_entity,
        }
    }

    pub fn before_entity(&self) -> Option<Entity> {
        self.before_entity
    }

    pub fn after_entity(&self) -> Option<Entity> {
        self.after_entity
    }
}

pub struct Transaction {
    // This is the snapshot of the world when the transaction starts
    before_world: legion::world::World,

    // This is the world that a downstream caller can manipulate. We will diff the data here against
    // the before_world to produce diffs
    after_world: legion::world::World,
}

#[derive(Clone)]
pub struct TransactionDiffs {
    apply_diff: WorldDiff,
    revert_diff: WorldDiff,
}

impl TransactionDiffs {
    pub fn new(
        apply_diff: WorldDiff,
        revert_diff: WorldDiff,
    ) -> Self {
        TransactionDiffs {
            apply_diff,
            revert_diff,
        }
    }

    pub fn apply_diff(&self) -> &WorldDiff {
        &self.apply_diff
    }

    pub fn revert_diff(&self) -> &WorldDiff {
        &self.revert_diff
    }

    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.apply_diff, &mut self.revert_diff);
    }
}

impl Transaction {
    pub fn world(&self) -> &World {
        &self.after_world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.after_world
    }

    pub fn create_transaction_diffs<S: BuildHasher>(
        &mut self,
        registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration, S>,
    ) -> TransactionDiffs {
        // These will contain the instructions to add/remove entities
        let mut apply_entity_diffs = vec![];
        let mut revert_entity_diffs = vec![];

        let mut all_entities = vec![];

        // Find the entities that have been deleted
        let mut all = Entity::query();
        for entity in all.iter(&self.before_world) {
            // Push all entities from the old world
            all_entities.push(*entity);

            if !self.after_world.contains(*entity) {
                let entity_uuid = self.before_world.universe().canon().get_name(*entity).unwrap();
                revert_entity_diffs.push(EntityDiff::new(entity_uuid, EntityDiffOp::Add));
                apply_entity_diffs.push(EntityDiff::new(entity_uuid, EntityDiffOp::Remove));
            }
        }

        // Find the entities that have been added
        for entity in all.iter(&self.after_world) {
            if !self.before_world.contains(*entity) {
                // Push all entities that were not in the old world. This combined with the previous
                // loop will ensure all_entities is a union with no duplicates of all entities in
                // the before world and after world
                all_entities.push(*entity);

                // Generate Add/Remove diffs
                let entity_uuid = self.before_world.universe().canon().get_name(*entity).unwrap();
                apply_entity_diffs.push(EntityDiff::new(entity_uuid,EntityDiffOp::Add));
                revert_entity_diffs.push(EntityDiff::new(entity_uuid,EntityDiffOp::Remove));
            }
        }

        // We detect which entities are new and old:
        // - Deleted entities we could skip in the below code since the component delete diffs are
        //   redundant, but we need to generate component adds in the undo world diff
        // - New entities also go through the below code to create component diffs. However this is
        //   suboptimal since adding the diffs could require multiple entity moves between
        //   archetypes.
        // - Modified entities can feed into the below code to generate component add/remove/change
        //   diffs. This is still a little suboptimal if multiple components are added, but it's
        //   likely not the common case and something we can try to do something about later

        let mut apply_component_diffs = vec![];
        let mut revert_component_diffs = vec![];

        // Iterate the entities in the selection world and prefab world and genereate diffs for
        // each component type.
        for entity in all_entities {
            let entity_uuid = self.before_world.universe().canon().get_name(entity).unwrap();
            // Do diffs for each component type
            for (component_type, registration) in registered_components {
                let mut apply_data = vec![];
                let mut apply_ser = bincode::Serializer::new(
                    &mut apply_data,
                    bincode::config::DefaultOptions::new(),
                );
                let mut apply_ser_erased = erased_serde::Serializer::erase(&mut apply_ser);

                let apply_result = registration.diff_single(
                    &mut apply_ser_erased,
                    &self.before_world,
                    Some(entity),
                    &self.after_world,
                    Some(entity),
                );

                if apply_result != DiffSingleResult::NoChange {
                    let mut revert_data = vec![];
                    let mut revert_ser = bincode::Serializer::new(
                        &mut revert_data,
                        bincode::config::DefaultOptions::new(),
                    );
                    let mut revert_ser_erased = erased_serde::Serializer::erase(&mut revert_ser);

                    let revert_result = registration.diff_single(
                        &mut revert_ser_erased,
                        &self.after_world,
                        Some(entity),
                        &self.before_world,
                        Some(entity),
                    );

                    apply_component_diffs.push(
                        ComponentDiff::new_from_diff_single_result(
                            entity_uuid,
                            *component_type,
                            apply_result,
                            apply_data,
                        )
                        .unwrap(),
                    );

                    revert_component_diffs.push(
                        ComponentDiff::new_from_diff_single_result(
                            entity_uuid,
                            *component_type,
                            revert_result,
                            revert_data,
                        )
                        .unwrap(),
                    );
                }
            }
        }

        let apply_diff = WorldDiff::new(apply_entity_diffs, apply_component_diffs);
        let revert_diff = WorldDiff::new(revert_entity_diffs, revert_component_diffs);

        TransactionDiffs::new(apply_diff, revert_diff)
    }
}
