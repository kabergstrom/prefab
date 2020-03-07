use legion::prelude::*;
use legion_prefab::ComponentRegistration;
use prefab_format::ComponentTypeUuid;
use serde::{Deserialize, Serialize};
use serde_diff::SerdeDiff;
use std::collections::HashMap;
use type_uuid::TypeUuid;

// Components require TypeUuid + Serialize + Deserialize + SerdeDiff + Send + Sync
#[derive(TypeUuid, Serialize, Deserialize, SerdeDiff, Clone, Default)]
#[uuid = "f5780013-bae4-49f0-ac0e-a108ff52fec0"]
struct Position2D {
    position: Vec<f32>,
}

legion_prefab::register_component_type!(Position2D);

mod prefab_sample {
    include!("test.prefab");
}
fn main() {
    let mut de = ron::de::Deserializer::from_str(prefab_sample::PREFAB).unwrap();

    // Create the component registry
    let registered_components = {
        let comp_registrations = legion_prefab::iter_component_registrations();
        use std::iter::FromIterator;
        let component_types: HashMap<ComponentTypeUuid, ComponentRegistration> =
            HashMap::from_iter(comp_registrations.map(|reg| (reg.uuid().clone(), reg.clone())));

        component_types
    };

    let prefab_serde_context = legion_prefab::PrefabSerdeContext {
        registered_components,
    };

    let prefab_deser = legion_prefab::PrefabFormatDeserializer::new(&prefab_serde_context);
    prefab_format::deserialize(&mut de, &prefab_deser).unwrap();

    let prefab = prefab_deser.prefab();
    println!("iterate positions");
    let query = <legion::prelude::Read<Position2D>>::query();
    for pos in query.iter(&prefab.world) {
        println!("position: {:?}", pos.position);
    }
    println!("done iterating positions");

    let legion_world_str =
        ron::ser::to_string_pretty(&prefab, ron::ser::PrettyConfig::default()).unwrap();
    println!(
        "Prefab world serialized with load/save optimized format: {}",
        legion_world_str
    );

    let mut ron_ser = ron::ser::Serializer::new(Some(ron::ser::PrettyConfig::default()), true);
    let prefab_ser = legion_prefab::PrefabFormatSerializer::new(&prefab_serde_context, &prefab);
    prefab_format::serialize(&mut ron_ser, &prefab_ser, prefab.prefab_id())
        .expect("failed to round-trip prefab");
    println!("Round-tripped prefab: {}", ron_ser.into_output_string());
}
