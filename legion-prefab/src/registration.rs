pub use inventory;
use legion::storage::{EntityLayout, /*ComponentMeta,*/ ComponentStorage};
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

type CompSerializeFn =
    unsafe fn(*const u8, &mut dyn FnMut(&dyn erased_serde::Serialize));
type CompDeserializeFn = fn(
    deserializer: &mut dyn erased_serde::Deserializer,
    get_next_storage_fn: &mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
) -> Result<(), erased_serde::Error>;
type CompRegisterFn = fn(&mut EntityLayout);
type DeserializeSingleFn = fn(
    &mut dyn erased_serde::Deserializer,
    &mut World,
    Entity,
);
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
type RemoveFromEntityFn = fn(
    &mut World,
    Entity,
);

#[derive(Clone)]
pub struct ComponentRegistration {
    pub(crate) component_type_id: ComponentTypeId,
    pub(crate) uuid: type_uuid::Bytes,
    pub(crate) ty: TypeId,
    //pub(crate) meta: ComponentMeta,
    pub(crate) type_name: &'static str,
    pub(crate) comp_serialize_fn: CompSerializeFn,
    pub(crate) comp_deserialize_fn: CompDeserializeFn,
    pub(crate) register_comp_fn: CompRegisterFn,
    pub(crate) deserialize_single_fn: DeserializeSingleFn,
    pub(crate) serialize_single_fn: SerializeSingleFn,
    pub(crate) diff_single_fn: DiffSingleFn,
    pub(crate) apply_diff_fn: ApplyDiffFn,
    pub(crate) comp_clone_fn: CompCloneFn,
    pub(crate) add_default_to_entity_fn: AddDefaultToEntityFn,
    pub(crate) remove_from_entity_fn: RemoveFromEntityFn,
}

impl ComponentRegistration {
    pub fn uuid(&self) -> &type_uuid::Bytes {
        &self.uuid
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn component_type_id(&self) -> ComponentTypeId {
        self.component_type_id
    }

    //pub fn meta(&self) -> &ComponentMeta {
    //     &self.meta
    // }

    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    pub fn deserialize_single(
        &self,
        deserializer: &mut dyn erased_serde::Deserializer,
        world: &mut legion::world::World,
        entity: Entity,
    ) {
        (self.deserialize_single_fn)(deserializer, world, entity)
    }

    pub fn add_default_to_entity(
        &self,
        world: &mut legion::world::World,
        entity: Entity,
    ) {
        (self.add_default_to_entity_fn)(world, entity)
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

    pub fn serialize(
        &self,
        world: &legion::world::World,
        entity: Entity,
        serialize: &mut dyn FnMut(&dyn erased_serde::Serialize),
    ) {
        (self.serialize_single_fn)(world, entity, serialize);
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
            comp_serialize_fn: |ptr, serialize_fn| unsafe {
                //let slice = comp_storage.get::<T>();
                let component_ptr = ptr as *const T;
                unsafe {
                    serialize_fn(&*component_ptr);
                }
            },
            comp_deserialize_fn: |deserializer, get_next_storage_fn| {
                let comp_seq_deser = ComponentSeqDeserializer::<T> {
                    get_next_storage_fn,
                    _marker: PhantomData,
                };
                comp_seq_deser.deserialize(deserializer)?;
                Ok(())
            },
            register_comp_fn: |desc| {
                desc.register_component::<T>();
            },
            deserialize_single_fn: |d,
                                    world,
                                    entity| {
                // TODO propagate error
                let comp =
                    erased_serde::deserialize::<T>(d).expect("failed to deserialize component");
                world.entry(entity).unwrap().add_component(comp);
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
