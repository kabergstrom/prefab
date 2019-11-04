pub const PREFAB1: &str = r#"Prefab(
    // Prefab AssetUuid
    id: "5fd8256d-db36-4fe2-8211-c7b3446e1927",
    objects: [
       // Embed the contents of another prefab in this prefab and override certain values
       PrefabRef((
             prefab_id: "14dec17f-ae14-40a3-8e44-e487fc423287",
             entity_overrides: [
                 (
                      entity_id: "62b3dbd1-56a8-469e-a262-41a66321da8b",
                      // Override values of a component in an entity of the referenced prefab
                      component_overrides: [
                          (
                              component_type: "d4b83227-d3f8-47f5-b026-db615fb41d31",
                              diff: [ Enter(Field("translation")), Enter(CollectionIndex(1)), Value(5.0) ]
                          ),
                      ],
                 ),
             ],
       ))
    ]
)"#;

pub const PREFAB2: &str = r#"Prefab(
    // Prefab AssetUuid
    id: "14dec17f-ae14-40a3-8e44-e487fc423287",
    objects: [
        // Inline definition of an entity and its components
        Entity(Entity(
             // Entity AssetUuid
             id: "62b3dbd1-56a8-469e-a262-41a66321da8b",
             // Component data and types
             components: [
                 (
                     // Component AssetTypeId
                     type: "d4b83227-d3f8-47f5-b026-db615fb41d31",
                     data: (
                         translation: [0.0, 0.0, 5.0],
                         scale: [2.0, 2.0, 2.0]
                     ),
                 ),
             ]
        )),
    ]
)"#;
