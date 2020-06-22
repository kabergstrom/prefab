use legion::prelude::*;
use prefab_format::{EntityUuid, ComponentTypeUuid};

use std::collections::HashMap;
use std::collections::HashSet;
use legion_prefab::{ComponentRegistration, DiffSingleResult};
use crate::component_diffs::{
    ComponentDiff, EntityDiff, EntityDiffOp, WorldDiff,
};
use legion_prefab::CopyCloneImpl;

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

    pub fn begin(
        self,
        universe: &Universe,
        src_world: &World,
        clone_impl: &CopyCloneImpl,
    ) -> Transaction {
        let mut before_world = universe.create_world();
        let mut after_world = universe.create_world();

        let mut uuid_to_entities = HashMap::new();

        for entity_info in self.entities {
            let before_entity =
                before_world.clone_from_single(&src_world, entity_info.entity, clone_impl, None);
            let after_entity =
                after_world.clone_from_single(&src_world, entity_info.entity, clone_impl, None);
            uuid_to_entities.insert(
                entity_info.entity_uuid,
                TransactionEntityInfo {
                    before_entity: Some(before_entity),
                    after_entity: Some(after_entity),
                },
            );
        }

        Transaction {
            before_world,
            after_world,
            uuid_to_entities,
        }
    }
}

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

    // All known entities throughout the transaction
    uuid_to_entities: HashMap<EntityUuid, TransactionEntityInfo>,
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

    pub fn uuid_to_entity(
        &self,
        uuid: EntityUuid,
    ) -> Option<Entity> {
        self.uuid_to_entities[&uuid].after_entity()
    }

    pub fn create_transaction_diffs(
        &mut self,
        registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration>,
    ) -> TransactionDiffs {
        log::trace!("create diffs for {} entities", self.uuid_to_entities.len());

        // These will contain the instructions to add/remove entities
        let mut apply_entity_diffs = vec![];
        let mut revert_entity_diffs = vec![];

        // Find the entities that have been deleted
        let mut preexisting_after_entities = HashSet::new();
        let mut removed_entity_uuids = HashSet::new();
        for (entity_uuid, entity_info) in &self.uuid_to_entities {
            if let Some(after_entity) = entity_info.after_entity {
                if self.after_world.get_entity_location(after_entity).is_none() {
                    removed_entity_uuids.insert(*entity_uuid);
                    apply_entity_diffs.push(EntityDiff::new(*entity_uuid, EntityDiffOp::Remove));

                    //TODO: This add wouldn't need to have an entity uuid with it, except that
                    // we may generate component diffs below. It would be more efficient to let the
                    // entity add contain all the data to create the entity anyways, but for a first
                    // pass we'll just let the component diffs do it
                    revert_entity_diffs.push(EntityDiff::new(*entity_uuid, EntityDiffOp::Add));
                }

                preexisting_after_entities.insert(after_entity);
            }
        }

        // Find the entities that have been added
        for after_entity in self.after_world.iter_entities() {
            if !preexisting_after_entities.contains(&after_entity) {
                let new_entity_uuid = uuid::Uuid::new_v4();

                apply_entity_diffs.push(EntityDiff::new(
                    *new_entity_uuid.as_bytes(),
                    EntityDiffOp::Add,
                ));

                revert_entity_diffs.push(EntityDiff::new(
                    *new_entity_uuid.as_bytes(),
                    EntityDiffOp::Remove,
                ));

                // Add new entities now so that the component diffing code will pick the new entity
                // and capture component data for it
                self.uuid_to_entities.insert(
                    *new_entity_uuid.as_bytes(),
                    TransactionEntityInfo::new(None, Some(after_entity)),
                );
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
        for (entity_uuid, entity_info) in &self.uuid_to_entities {
            // Do diffs for each component type
            for (component_type, registration) in registered_components {
                let mut apply_data = vec![];
                let mut apply_ser = bincode::Serializer::new(&mut apply_data, bincode::config::DefaultOptions::new());
                let mut apply_ser_erased = erased_serde::Serializer::erase(&mut apply_ser);

                let apply_result = registration.diff_single(
                    &mut apply_ser_erased,
                    &self.before_world,
                    entity_info.before_entity,
                    &self.after_world,
                    entity_info.after_entity,
                );

                if apply_result != DiffSingleResult::NoChange {
                    let mut revert_data = vec![];
                    let mut revert_ser = bincode::Serializer::new(&mut revert_data, bincode::config::DefaultOptions::new());
                    let mut revert_ser_erased = erased_serde::Serializer::erase(&mut revert_ser);

                    let revert_result = registration.diff_single(
                        &mut revert_ser_erased,
                        &self.after_world,
                        entity_info.after_entity,
                        &self.before_world,
                        entity_info.before_entity,
                    );

                    apply_component_diffs.push(
                        ComponentDiff::new_from_diff_single_result(
                            *entity_uuid,
                            *component_type,
                            apply_result,
                            apply_data,
                        )
                        .unwrap(),
                    );

                    revert_component_diffs.push(
                        ComponentDiff::new_from_diff_single_result(
                            *entity_uuid,
                            *component_type,
                            revert_result,
                            revert_data,
                        )
                        .unwrap(),
                    );
                }
            }
        }

        // We delayed removing entities from uuid_to_entities because we still want to generate add
        // entries for the undo step
        for removed_entity_uuid in &removed_entity_uuids {
            self.uuid_to_entities.remove(removed_entity_uuid);
        }

        let apply_diff = WorldDiff::new(apply_entity_diffs, apply_component_diffs);
        let revert_diff = WorldDiff::new(revert_entity_diffs, revert_component_diffs);

        TransactionDiffs::new(apply_diff, revert_diff)
    }
}
