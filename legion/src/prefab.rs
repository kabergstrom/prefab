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
struct Prefab {
    id: PrefabUuid,
    prefab_refs: HashMap<PrefabUuid, PrefabRef>,
    entities: HashMap<EntityUuid, legion::entity::Entity>,
}

    fn instantiate()