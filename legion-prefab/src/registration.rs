pub use inventory;
use legion::storage::{ArchetypeDescription, ComponentMeta, ComponentResourceSet, TagStorage};
use serde::{
    de::{self, DeserializeSeed, IgnoredAny, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_diff::SerdeDiff;
use std::{any::TypeId, marker::PhantomData, ptr::NonNull};
use type_uuid::TypeUuid;

struct ComponentDeserializer<'de, T: Deserialize<'de>> {
    ptr: *mut T,
    _marker: PhantomData<&'de T>,
}

impl<'de, T: Deserialize<'de> + 'static> DeserializeSeed<'de> for ComponentDeserializer<'de, T> {
    type Value = ();
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
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
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
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

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("sequence of objects")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
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

#[derive(Clone)]
pub struct TagRegistration {
    pub(crate) uuid: type_uuid::Bytes,
    pub(crate) ty: TypeId,
    pub(crate) tag_serialize_fn: fn(&TagStorage, &mut dyn FnMut(&dyn erased_serde::Serialize)),
    pub(crate) tag_deserialize_fn: fn(
        deserializer: &mut dyn erased_serde::Deserializer,
        &mut TagStorage,
    ) -> Result<(), erased_serde::Error>,
    pub(crate) register_tag_fn: fn(&mut ArchetypeDescription),
}

impl TagRegistration {
    pub fn uuid(&self) -> &type_uuid::Bytes {
        &self.uuid
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn of<
        T: TypeUuid
            + Serialize
            + for<'de> Deserialize<'de>
            + PartialEq
            + Clone
            + Send
            + Sync
            + 'static,
    >() -> Self {
        Self {
            uuid: T::UUID,
            ty: TypeId::of::<T>(),
            tag_serialize_fn: |tag_storage, serialize_fn| {
                // it's safe because we know this is the correct type due to lookup
                let slice = unsafe { tag_storage.data_slice::<T>() };
                serialize_fn(&&*slice);
            },
            tag_deserialize_fn: |deserializer, tag_storage| {
                // TODO implement visitor to avoid allocation of Vec
                let tag_vec = <Vec<T> as Deserialize>::deserialize(deserializer)?;
                for tag in tag_vec {
                    // Tag types should line up, making this safe
                    unsafe {
                        tag_storage.push(tag);
                    }
                }
                Ok(())
            },
            register_tag_fn: |desc| {
                desc.register_tag::<T>();
            },
        }
    }
}

#[derive(Clone)]
pub struct ComponentRegistration {
    pub(crate) uuid: type_uuid::Bytes,
    pub(crate) ty: TypeId,
    pub(crate) meta: ComponentMeta,
    pub(crate) comp_serialize_fn:
        unsafe fn(&ComponentResourceSet, &mut dyn FnMut(&dyn erased_serde::Serialize)),
    pub(crate) comp_deserialize_fn: fn(
        deserializer: &mut dyn erased_serde::Deserializer,
        get_next_storage_fn: &mut dyn FnMut() -> Option<(NonNull<u8>, usize)>,
    ) -> Result<(), erased_serde::Error>,
    pub(crate) register_comp_fn: fn(&mut ArchetypeDescription),
    pub(crate) deserialize_single_fn:
        fn(&mut dyn erased_serde::Deserializer, &mut legion::world::World, legion::entity::Entity),
    pub(crate) serialize_single_fn: fn(
        &legion::world::World,
        legion::entity::Entity,
        &mut dyn FnMut(&dyn erased_serde::Serialize),
    ),
    pub(crate) diff_single: fn(
        &mut dyn erased_serde::Serializer,
        &legion::world::World,
        legion::entity::Entity,
        &legion::world::World,
        legion::entity::Entity,
    ),
    pub(crate) apply_diff:
        fn(&mut dyn erased_serde::Deserializer, &mut legion::world::World, legion::entity::Entity),
    pub(crate) comp_clone_fn: fn(*const u8, *mut u8, usize),
}

impl ComponentRegistration {
    pub fn uuid(&self) -> &type_uuid::Bytes {
        &self.uuid
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn meta(&self) -> &ComponentMeta {
        &self.meta
    }

    pub fn diff_single(
        &self,
        ser: &mut dyn erased_serde::Serializer,
        src_world: &legion::world::World,
        src_entity: legion::entity::Entity,
        dst_world: &legion::world::World,
        dst_entity: legion::entity::Entity,
    ) {
        (self.diff_single)(ser, src_world, src_entity, dst_world, dst_entity)
    }

    pub fn apply_diff(
        &self,
        de: &mut dyn erased_serde::Deserializer,
        world: &mut legion::world::World,
        entity: legion::entity::Entity,
    ) {
        (self.apply_diff)(de, world, entity);
    }

    pub unsafe fn clone_components(&self, src: *const u8, dst: *mut u8, num_components: usize) {
        (self.comp_clone_fn)(src, dst, num_components);
    }

    pub fn serialize(
        &self,
        world: &legion::world::World,
        entity: legion::entity::Entity,
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
            + 'static,
    >() -> Self {
        Self {
            uuid: T::UUID,
            ty: TypeId::of::<T>(),
            meta: ComponentMeta::of::<T>(),
            comp_serialize_fn: |comp_storage, serialize_fn| unsafe {
                let slice = comp_storage.data_slice::<T>();
                serialize_fn(&*slice);
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
            deserialize_single_fn: |d, world, entity| {
                // TODO propagate error
                let comp =
                    erased_serde::deserialize::<T>(d).expect("failed to deserialize component");
                world.add_component(entity, comp);
            },
            serialize_single_fn: |world, entity, s_fn| {
                let comp = world
                    .get_component::<T>(entity)
                    .expect("entity not present when serializing component");
                s_fn(&*comp)
            },
            diff_single: |ser, src_world, src_entity, dst_world, dst_entity| {
                // TODO propagate error
                let mut src_comp = src_world
                    .get_component::<T>(src_entity);
                let mut dst_comp = dst_world
                    .get_component::<T>(dst_entity);

                if let (Some(src_comp), Some(dst_comp)) = (src_comp, dst_comp) {

                    <serde_diff::Diff<T> as serde::ser::Serialize>::serialize(
                        &serde_diff::Diff::serializable(&*src_comp, &*dst_comp),
                        ser
                    ).expect("failed to serialize diff");
                }


            },
            apply_diff: |d, world, entity| {
                // TODO propagate error
                let mut comp = world
                    .get_component_mut::<T>(entity)
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
        }
    }
}

inventory::collect!(TagRegistration);
inventory::collect!(ComponentRegistration);

pub fn iter_component_registrations() -> impl Iterator<Item = &'static ComponentRegistration> {
    inventory::iter::<ComponentRegistration>.into_iter()
}
pub fn iter_tag_registrations() -> impl Iterator<Item = &'static TagRegistration> {
    inventory::iter::<TagRegistration>.into_iter()
}

#[macro_export]
macro_rules! register_tag_type {
    ($tag_type:ty) => {
        $crate::register_tag_type!(legion_prefab; $tag_type);
    };
    ($krate:ident; $tag_type:ty) => {
        $crate::inventory::submit!{
            #![crate = $krate]
            $crate::TagRegistration::of::<$tag_type>()
        }
    };
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
