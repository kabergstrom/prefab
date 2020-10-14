use prefab_format::{ComponentTypeUuid, EntityUuid};
use legion_prefab::CookedPrefab;
use legion_prefab::Prefab;
use std::collections::HashMap;
use legion::*;
use legion_prefab::DiffSingleResult;
use legion_prefab::ComponentRegistration;
use legion_prefab::CopyCloneImpl;
use std::hash::BuildHasher;

#[derive(Clone, Debug)]
pub enum EntityDiffOp {
    Add,
    Remove,
}

#[derive(Clone, Debug)]
pub struct EntityDiff {
    entity_uuid: EntityUuid,
    op: EntityDiffOp,
}

impl EntityDiff {
    pub fn new(
        entity_uuid: EntityUuid,
        op: EntityDiffOp,
    ) -> Self {
        EntityDiff { entity_uuid, op }
    }

    pub fn entity_uuid(&self) -> &EntityUuid {
        &self.entity_uuid
    }

    pub fn op(&self) -> &EntityDiffOp {
        &self.op
    }
}

// This is somewhat of a mirror of DiffSingleResult
#[derive(Clone, Debug)]
pub enum ComponentDiffOp {
    Change(Vec<u8>),
    Add(Vec<u8>),
    Remove,
}

impl ComponentDiffOp {
    pub fn from_diff_single_result(
        diff_single_result: DiffSingleResult,
        data: Vec<u8>,
    ) -> Option<ComponentDiffOp> {
        match diff_single_result {
            DiffSingleResult::Add => Some(ComponentDiffOp::Add(data)),
            DiffSingleResult::Change => Some(ComponentDiffOp::Change(data)),
            DiffSingleResult::Remove => Some(ComponentDiffOp::Remove),
            DiffSingleResult::NoChange => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ComponentDiff {
    entity_uuid: EntityUuid,
    component_type: ComponentTypeUuid,
    op: ComponentDiffOp,
}

impl ComponentDiff {
    pub fn new(
        entity_uuid: EntityUuid,
        component_type: ComponentTypeUuid,
        op: ComponentDiffOp,
    ) -> Self {
        ComponentDiff {
            entity_uuid,
            component_type,
            op,
        }
    }

    pub fn new_from_diff_single_result(
        entity_uuid: EntityUuid,
        component_type: ComponentTypeUuid,
        diff_single_result: DiffSingleResult,
        data: Vec<u8>,
    ) -> Option<Self> {
        let op = ComponentDiffOp::from_diff_single_result(diff_single_result, data);
        op.map(|op| Self::new(entity_uuid, component_type, op))
    }

    pub fn entity_uuid(&self) -> &EntityUuid {
        &self.entity_uuid
    }

    pub fn component_type(&self) -> &ComponentTypeUuid {
        &self.component_type
    }

    pub fn op(&self) -> &ComponentDiffOp {
        &self.op
    }
}

#[derive(Clone, Debug)]
pub struct WorldDiff {
    entity_diffs: Vec<EntityDiff>,
    component_diffs: Vec<ComponentDiff>,
}

impl WorldDiff {
    pub fn new(
        entity_diffs: Vec<EntityDiff>,
        component_diffs: Vec<ComponentDiff>,
    ) -> WorldDiff {
        WorldDiff {
            entity_diffs,
            component_diffs,
        }
    }

    pub fn has_changes(&self) -> bool {
        !self.entity_diffs.is_empty() || !self.component_diffs.is_empty()
    }

    pub fn entity_diffs(&self) -> &Vec<EntityDiff> {
        &self.entity_diffs
    }

    pub fn component_diffs(&self) -> &Vec<ComponentDiff> {
        &self.component_diffs
    }
}

#[derive(Debug)]
pub enum ApplyDiffToPrefabError {
    PrefabHasOverrides,
}

/// Applies a world diff to a prefab
///
/// This is currently only supported for prefabs that have no overrides. If there is an override,
/// None will be returned
pub fn apply_diff_to_prefab<S: BuildHasher, T: BuildHasher>(
    prefab: &Prefab,
    diff: &WorldDiff,
    registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration, T>,
    clone_impl: CopyCloneImpl<S>,
) -> Result<Prefab, ApplyDiffToPrefabError> {
    if !prefab.prefab_meta.prefab_refs.is_empty() {
        return Err(ApplyDiffToPrefabError::PrefabHasOverrides);
    }

    let (new_world, uuid_to_new_entities) = apply_diff(
        &prefab.world,
        &prefab.prefab_meta.entities,
        diff,
        registered_components,
        clone_impl,
    );

    let prefab_meta = legion_prefab::PrefabMeta {
        id: prefab.prefab_meta.id,
        prefab_refs: Default::default(),
        entities: uuid_to_new_entities,
    };

    Ok(legion_prefab::Prefab {
        world: new_world,
        prefab_meta,
    })
}

/// Applies a world diff to a cooked prefab
pub fn apply_diff_to_cooked_prefab<S: BuildHasher, T: BuildHasher>(
    cooked_prefab: &CookedPrefab,
    diff: &WorldDiff,
    registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration, T>,
    clone_impl: CopyCloneImpl<S>,
) -> CookedPrefab {
    let (new_world, uuid_to_new_entities) = apply_diff(
        &cooked_prefab.world,
        &cooked_prefab.entities,
        diff,
        registered_components,
        clone_impl,
    );

    CookedPrefab {
        world: new_world,
        entities: uuid_to_new_entities,
    }
}

pub fn apply_diff<S: BuildHasher, U: BuildHasher, T: BuildHasher>(
    world: &World,
    uuid_to_entity: &HashMap<EntityUuid, Entity, T>,
    diff: &WorldDiff,
    registered_components: &HashMap<ComponentTypeUuid, ComponentRegistration, U>,
    mut clone_impl: CopyCloneImpl<S>,
) -> (World, HashMap<EntityUuid, Entity>) {
    // Create an empty world to populate
    let mut new_world = World::default();

    // Copy everything from the opened prefab into the new world as a baseline
    let result_mappings = new_world.clone_from(world, &legion::query::any(), &mut clone_impl);

    let mut uuid_to_new_entities = HashMap::default();
    for (uuid, prefab_entity) in uuid_to_entity {
        let new_world_entity = result_mappings.get(prefab_entity).unwrap();
        uuid_to_new_entities.insert(*uuid, *new_world_entity);
    }

    for entity_diff in &diff.entity_diffs {
        match entity_diff.op() {
            EntityDiffOp::Add => {
                let new_entity = new_world.extend(vec![()]);
                uuid_to_new_entities.insert(*entity_diff.entity_uuid(), new_entity[0]);
            }
            EntityDiffOp::Remove => {
                if let Some(new_prefab_entity) = uuid_to_new_entities.get(entity_diff.entity_uuid())
                {
                    new_world.remove(*new_prefab_entity);
                    uuid_to_new_entities.remove(entity_diff.entity_uuid());
                } else {
                    //TODO: Produce a remove override
                }
            }
        }
    }

    for component_diff in &diff.component_diffs {
        if let Some(new_prefab_entity) = uuid_to_new_entities.get(component_diff.entity_uuid()) {
            if let Some(component_registration) =
                registered_components.get(component_diff.component_type())
            {
                match component_diff.op() {
                    ComponentDiffOp::Change(data) => {
                        //TODO: Detect if we need to make the change in the world or as an override
                        let mut deserializer =
                            bincode::Deserializer::<bincode::de::read::SliceReader, _>::from_slice(
                                data.as_slice(),
                                bincode::config::DefaultOptions::new(),
                            );
                        let mut de_erased = erased_serde::Deserializer::erase(&mut deserializer);

                        component_registration.apply_diff(
                            &mut de_erased,
                            &mut new_world,
                            *new_prefab_entity,
                        );
                    }
                    ComponentDiffOp::Add(data) => {
                        //TODO: Detect if we need to make the change in the world or as an override
                        let mut deserializer =
                            bincode::Deserializer::<bincode::de::read::SliceReader, _>::from_slice(
                                data,
                                bincode::config::DefaultOptions::new(),
                            );
                        let mut de_erased = erased_serde::Deserializer::erase(&mut deserializer);

                        component_registration.add_to_entity(
                            &mut de_erased,
                            &mut new_world,
                            *new_prefab_entity,
                        );
                    }
                    ComponentDiffOp::Remove => {
                        //TODO: Detect if we need to make the change in the world or as an override
                        //TODO: propagate error
                        component_registration
                            .remove_from_entity(&mut new_world, *new_prefab_entity);
                    }
                }
            }
        }
    }

    (new_world, uuid_to_new_entities)
}
