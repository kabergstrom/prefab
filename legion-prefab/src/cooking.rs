
use legion::prelude::*;
use legion::storage::ComponentTypeId;
use std::collections::HashMap;
use crate::{CookedPrefab, Prefab, ComponentRegistration, CopyCloneImpl};
use prefab_format::{PrefabUuid, ComponentTypeUuid};

pub fn cook_prefab(
    universe: &Universe,
    registered_components: &HashMap<ComponentTypeId, ComponentRegistration>,
    registered_components_by_uuid: &HashMap<ComponentTypeUuid, ComponentRegistration>,
    prefab_cook_order: &[PrefabUuid],
    prefab_lookup: &HashMap<PrefabUuid, &Prefab>,
) -> CookedPrefab {
    // Create the clone_merge impl. For prefab cooking, we will clone everything so we don't need to
    // set up any transformations
    let clone_merge_impl = CopyCloneImpl::new(registered_components);

    // This will allow us to look up the cooked entity ID by the entity's original UUID
    let mut entity_lookup = HashMap::new();
    // Create a new world to hold the cooked data
    let mut world = universe.create_world();
    // merge all entity data from all prefabs. This data doesn't include any overrides, so order
    // doesn't matter
    for (_, prefab) in prefab_lookup {
        // Clone all the entities from the prefab into the cooked world. As the data is copied,
        // entity will get a new Entity assigned to it in the cooked world. result_mappings will
        // be populated as this happens so that we can trace where data in the prefab landed in
        // the cooked world
        let mut result_mappings = HashMap::new();
        world.clone_from(
            &prefab.world,
            &clone_merge_impl,
            &mut legion::world::HashMapCloneImplResult(&mut result_mappings),
            &legion::world::NoneEntityReplacePolicy,
        );

        // Iterate the entities in this prefab. Determine where they are stored in the cooked
        // world and store this in entity_lookup
        for (entity_uuid, prefab_entity) in &prefab.prefab_meta.entities {
            let cooked_entity = result_mappings[prefab_entity];
            entity_lookup.insert(*entity_uuid, cooked_entity);
        }
    }

    // apply component override data. iteration of prefabs is in order such that "base" prefabs
    // are processed first
    for prefab_id in prefab_cook_order {
        // fetch the data for the prefab
        let prefab = prefab_lookup[prefab_id];

        // Iterate all the other prefabs that this prefab references
        for (dependency_prefab_id, dependency_prefab_ref) in
        &prefab.prefab_meta.prefab_refs
        {
            // Iterate all the entities for which we have override data
            for (entity_id, component_overrides) in &dependency_prefab_ref.overrides {

                // Find where this entity is stored within the cooked data
                let cooked_entity = entity_lookup[entity_id];

                // Iterate all the component types for which we have override data
                for component_override in component_overrides {
                    let component_registration =
                        &registered_components_by_uuid[&component_override.component_type];

                    let mut deserializer =
                        ron::de::Deserializer::from_str(&component_override.data).unwrap();

                    let mut de = erased_serde::Deserializer::erase(&mut deserializer);
                    component_registration.apply_diff(&mut de, &mut world, cooked_entity);
                }
            }
        }
    }

    // the resulting world can now be saved
    crate::CookedPrefab {
        world: world,
        entities: entity_lookup,
    }
}