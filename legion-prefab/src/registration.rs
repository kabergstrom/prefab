pub use inventory;
use legion::storage::{EntityLayout, ComponentStorage, UnknownComponentStorage, ArchetypeIndex};
use serde::{
    de::{self, DeserializeSeed, IgnoredAny, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_diff::SerdeDiff;
use std::{any::TypeId, marker::PhantomData, ptr::NonNull};
use type_uuid::TypeUuid;
use legion::storage::ComponentTypeId;
use legion::EntityStore;
use legion::world::{Entity, World};

struct ComponentDeserializer<'de, T: Deserialize<'de>> {
    ptr: *mut T,
    _marker: PhantomData<&'de T>,
}

impl<'de, T: Deserialize<'de> + 'static> DeserializeSeed<'de> for ComponentDeserializer<'de, T> {
    type Value = ();
    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = <T as Deserialize<'de>>::deserialize(deserializer)?;
        unsafe {
            std::ptr::write(self.ptr, value);
        }
        Ok(())
    }
}

struct ComponentSeqDeserializer<'a, T> {
    get_next_storage_fn: &'a mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
    _marker: PhantomData<T>,
}

impl<'de, 'a, T: for<'b> Deserialize<'b> + 'static> DeserializeSeed<'de>
    for ComponentSeqDeserializer<'a, T>
{
    type Value = ();
    fn deserialize<D>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}
impl<'de, 'a, T: for<'b> Deserialize<'b> + 'static> Visitor<'de>
    for ComponentSeqDeserializer<'a, T>
{
    type Value = ();

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("sequence of objects")
    }
    fn visit_seq<A>(
        self,
        mut seq: A,
    ) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let size = seq.size_hint();
        for _ in 0..size.unwrap_or(std::usize::MAX) {
            match (self.get_next_storage_fn)() {
                Some((storage_ptr, storage_len)) => {
                    let storage_ptr = storage_ptr.as_ptr() as *mut T;
                    for idx in 0..storage_len {
                        let element_ptr = unsafe { storage_ptr.add(idx) };

                        if seq
                            .next_element_seed(ComponentDeserializer {
                                ptr: element_ptr,
                                _marker: PhantomData,
                            })?
                            .is_none()
                        {
                            panic!(
                                "expected {} elements in chunk but only {} found",
                                storage_len, idx
                            );
                        }
                    }
                }
                None => {
                    if seq.next_element::<IgnoredAny>()?.is_some() {
                        panic!("unexpected element when there was no storage space available");
                    } else {
                        // No more elements and no more storage - that's what we want!
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(PartialEq)]
pub enum DiffSingleResult {
    NoChange,
    Change,
    Add,
    Remove,
}

type CompRegisterFn = fn(&mut EntityLayout);

type CompSerializeFn =
    unsafe fn(*const u8, &mut dyn FnMut(&dyn erased_serde::Serialize));
type CompDeserializeFn = fn(
    storage: &mut UnknownComponentStorage,
    arch_index: ArchetypeIndex,
    deserializer: &mut dyn erased_serde::Deserializer,
    //get_next_storage_fn: &mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
) -> Result<(), erased_serde::Error>;

type DeserializeSingleFn = fn(
    &mut dyn erased_serde::Deserializer,
    //&mut World,
    //Entity,
) -> Result<Box<[u8]>, erased_serde::Error>;
type SerializeSingleFn =
    fn(&World, Entity, &mut dyn FnMut(&dyn erased_serde::Serialize));

type DiffSingleFn = fn(
    &mut dyn erased_serde::Serializer,
    &World,
    Option<Entity>,
    &World,
    Option<Entity>,
) -> DiffSingleResult;
type ApplyDiffFn =
    fn(&mut dyn erased_serde::Deserializer, &mut World, Entity);

type CompCloneFn = fn(*const u8, *mut u8, usize);
type AddDefaultToEntityFn = fn(
    &mut World,
    Entity,
);
type AddToEntityFn = fn(
    &mut dyn erased_serde::Deserializer,
    &mut World,
    Entity,
);
type RemoveFromEntityFn = fn(
    &mut World,
    Entity,
);

#[derive(Clone)]
pub struct ComponentRegistration {
    component_type_id: ComponentTypeId,
    uuid: type_uuid::Bytes,
    ty: TypeId,
    type_name: &'static str,
    register_comp_fn: CompRegisterFn,

    // These are used by legion to serialize worlds
    comp_serialize_fn: CompSerializeFn,
    comp_deserialize_fn: CompDeserializeFn,
    deserialize_single_fn: DeserializeSingleFn,

    // Used by prefab logic
    serialize_single_fn: SerializeSingleFn,
    diff_single_fn: DiffSingleFn,
    apply_diff_fn: ApplyDiffFn,
    comp_clone_fn: CompCloneFn,
    add_default_to_entity_fn: AddDefaultToEntityFn,
    add_to_entity_fn: AddToEntityFn,
    remove_from_entity_fn: RemoveFromEntityFn,
}

impl ComponentRegistration {
    pub fn component_type_id(&self) -> ComponentTypeId {
        self.component_type_id
    }

    pub fn uuid(&self) -> &type_uuid::Bytes {
        &self.uuid
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn register_component(&self, layout: &mut EntityLayout) {
        (self.register_comp_fn)(layout);
    }

    pub unsafe fn comp_serialize(
        &self,
        ptr: *const u8,
        serialize_fn: &mut dyn FnMut(&dyn erased_serde::Serialize)
    ) {
        (self.comp_serialize_fn)(ptr, serialize_fn)
    }

    pub fn comp_deserialize(
        &self,
        storage: &mut UnknownComponentStorage,
        arch_index: ArchetypeIndex,
        deserializer: &mut dyn erased_serde::Deserializer,
    ) -> Result<(), erased_serde::Error> {
        (self.comp_deserialize_fn)(storage, arch_index, deserializer)
    }

    pub fn deserialize_single(
        &self,
        deserializer: &mut dyn erased_serde::Deserializer,
    ) -> Result<Box<[u8]>, erased_serde::Error> {
        (self.deserialize_single_fn)(deserializer)
    }

    pub fn serialize_single(
        &self,
        world: &legion::world::World,
        entity: Entity,
        serialize: &mut dyn FnMut(&dyn erased_serde::Serialize),
    ) {
        (self.serialize_single_fn)(world, entity, serialize);
    }

    pub fn add_default_to_entity(
        &self,
        world: &mut legion::world::World,
        entity: Entity,
    ) {
        (self.add_default_to_entity_fn)(world, entity)
    }

    pub fn add_to_entity(
        &self,
        deserializer: &mut dyn erased_serde::Deserializer,
        world: &mut legion::world::World,
        entity: Entity,
    ) {
        (self.add_to_entity_fn)(deserializer, world, entity)
    }

    pub fn remove_from_entity(
        &self,
        world: &mut legion::world::World,
        entity: Entity,
    ) {
        (self.remove_from_entity_fn)(world, entity)
    }

    pub fn diff_single(
        &self,
        ser: &mut dyn erased_serde::Serializer,
        src_world: &legion::world::World,
        src_entity: Option<Entity>,
        dst_world: &legion::world::World,
        dst_entity: Option<Entity>,
    ) -> DiffSingleResult {
        (self.diff_single_fn)(ser, src_world, src_entity, dst_world, dst_entity)
    }

    pub fn apply_diff(
        &self,
        de: &mut dyn erased_serde::Deserializer,
        world: &mut legion::world::World,
        entity: Entity,
    ) {
        (self.apply_diff_fn)(de, world, entity);
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn clone_components(
        &self,
        src: *const u8,
        dst: *mut u8,
        num_components: usize,
    ) {
        (self.comp_clone_fn)(src, dst, num_components);
    }

    pub fn of<
        T: TypeUuid
            + Clone
            + Serialize
            + SerdeDiff
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Default
            + 'static,
    >() -> Self {
        Self {
            component_type_id: ComponentTypeId::of::<T>(),
            uuid: T::UUID,
            ty: TypeId::of::<T>(),
            //meta: ComponentMeta::of::<T>(),
            type_name: std::any::type_name::<T>(),
            register_comp_fn: |layout| {
                layout.register_component::<T>();
            },
            comp_serialize_fn: |ptr, serialize_fn| unsafe {
                let component_ptr = ptr as *const T;
                unsafe {
                    serialize_fn(&*component_ptr);
                }
            },
            comp_deserialize_fn: |
                storage,
                arch_index,
                deserializer,
            | {
                let mut components = erased_serde::deserialize::<Vec<T>>(deserializer)?;
                unsafe {
                    let ptr = components.as_ptr();
                    storage.extend_memcopy_raw(arch_index, ptr as *const u8, components.len());
                    components.set_len(0);
                }
                Ok(())
            },
            deserialize_single_fn: |d,
                                    //world,
                                    //entity
            | {
                let component = erased_serde::deserialize::<T>(d)?;
                unsafe {
                    let vec = std::slice::from_raw_parts(
                        &component as *const T as *const u8,
                        std::mem::size_of::<T>(),
                    ).to_vec();
                    std::mem::forget(component);
                    Ok(vec.into_boxed_slice())
                }
            },
            serialize_single_fn: |world, entity, s_fn| {
                let comp = world
                    .entry_ref(entity)
                    .unwrap();

                s_fn(
                    comp
                        .get_component::<T>()
                        .expect("entity not present when serializing component")
                );
            },
            diff_single_fn: |ser, src_world, src_entity, dst_world, dst_entity| {
                // TODO propagate error
                let src_comp = src_entity.and_then(|e| src_world.entry_ref(e));//.get_component::<T>());
                let dst_comp = dst_entity.and_then(|e| dst_world.entry_ref(e));//.get_component::<T>());

                if let (Some(src_comp), Some(dst_comp)) = (&src_comp, &dst_comp) {
                    //
                    // Component exists before and after the change. If differences exist, serialize
                    // a diff and return a Change result. Otherwise, serialize nothing and return
                    // NoChange
                    //
                    let diff = serde_diff::Diff::serializable(src_comp.get_component::<T>().unwrap(), dst_comp.get_component::<T>().unwrap());
                    <serde_diff::Diff<T> as serde::ser::Serialize>::serialize(&diff, ser)
                        .expect("failed to serialize diff");

                    if diff.has_changes() {
                        DiffSingleResult::Change
                    } else {
                        DiffSingleResult::NoChange
                    }
                } else if let Some(dst_comp) = &dst_comp {
                    //
                    // Component was created, serialize the object and return an Add result
                    //
                    erased_serde::serialize(dst_comp.get_component::<T>().unwrap(), ser).unwrap();
                    DiffSingleResult::Add
                } else if src_comp.is_some() {
                    //
                    // Component was removed, do not serialize anything and return a Remove result
                    //
                    DiffSingleResult::Remove
                } else {
                    //
                    // Component didn't exist before or after, so do nothing
                    //
                    DiffSingleResult::NoChange
                }
            },
            apply_diff_fn: |d, world, entity| {
                // TODO propagate error
                let mut e = world
                    .entry(entity)
                    .unwrap();

                let comp = e
                    .get_component_mut::<T>()
                    .expect("expected component data when diffing");
                let comp: &mut T = &mut *comp;
                <serde_diff::Apply<T> as serde::de::DeserializeSeed>::deserialize(
                    serde_diff::Apply::deserializable(comp),
                    d,
                )
                .expect("failed to deserialize diff");
            },
            comp_clone_fn: |src, dst, num_components| unsafe {
                for i in 0..num_components {
                    let src_ptr = (src as *const T).add(i);
                    let dst_ptr = (dst as *mut T).add(i);
                    std::ptr::write(dst_ptr, <T as Clone>::clone(&*src_ptr));
                }
            },
            add_default_to_entity_fn: |world, entity| world.entry(entity).unwrap().add_component(T::default()),
            add_to_entity_fn: |d,
                                    world,
                                    entity
            | {
                // TODO propagate error
                let comp =
                    erased_serde::deserialize::<T>(d).expect("failed to deserialize component");
                world.entry(entity).unwrap().add_component(comp);
            },
            remove_from_entity_fn: |world, entity| world.entry(entity).unwrap().remove_component::<T>(),
        }
    }
}

inventory::collect!(ComponentRegistration);

pub fn iter_component_registrations() -> impl Iterator<Item = &'static ComponentRegistration> {
    inventory::iter::<ComponentRegistration>.into_iter()
}

#[macro_export]
macro_rules! register_component_type {
    ($component_type:ty) => {
        $crate::register_component_type!(legion_prefab; $component_type);
    };
    ($krate:ident; $component_type:ty) => {
        $crate::inventory::submit!{
            #![crate = $krate]
            $crate::ComponentRegistration::of::<$component_type>()
        }
    };
}
