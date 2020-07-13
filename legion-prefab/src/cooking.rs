use legion::*;
use legion::storage::ComponentTypeId;
use std::collections::HashMap;
use crate::{CookedPrefab, Prefab, ComponentRegistration, CopyCloneImpl};
use prefab_format::{PrefabUuid, ComponentTypeUuid};
use std::hash::BuildHasher;

pub fn cook_prefab<S: BuildHasher, T: BuildHasher, U: BuildHasher>(
    universe: &Universe,
    registered_components: &HashMap<ComponentTypeId, ComponentRegistration, S>,
    registered_components_by_uuid: &HashMap<ComponentTypeUuid, ComponentRegistration, T>,
    prefab_cook_order: &[PrefabUuid],
    prefab_lookup: &HashMap<PrefabUuid, &Prefab, U>,
) -> CookedPrefab {
    // Create a new world to hold the cooked data
    let mut world = universe.create_world();
    // merge all entity data from all prefabs. This data doesn't include any overrides, so order
    // doesn't matter
    for prefab in prefab_lookup.values() {
        // Create the clone_merge impl. For prefab cooking, we will clone everything so we don't need to
        // set up any transformations
        let mut clone_merge_impl = CopyCloneImpl::new(registered_components);

        // Clone all the entities from the prefab into the cooked world.
        world.clone_from(
            &prefab.world,
            &legion::query::any(),
            &mut clone_merge_impl
        );
    }

    // apply component override data. iteration of prefabs is in order such that "base" prefabs
    // are processed first
    for prefab_id in prefab_cook_order {
        // fetch the data for the prefab
        let prefab = prefab_lookup[prefab_id];

        // Iterate all the other prefabs that this prefab references
        for dependency_prefab_ref in prefab.prefab_meta.prefab_refs.values() {
            // Iterate all the entities for which we have override data
            for (entity_uuid, component_overrides) in &dependency_prefab_ref.overrides {

                // Find where this entity is stored within the cooked data
                let cooked_entity = universe.canon().get_id(entity_uuid).unwrap();

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
        world,
    }
}
