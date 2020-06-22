use std::collections::HashMap;
use crate::ComponentRegistration;
use legion::storage::{ComponentMeta, ComponentTypeId, Component, ComponentStorage};
use legion::prelude::*;
use std::mem::MaybeUninit;
use std::ops::Range;
use legion::index::ComponentIndex;

/// A trivial clone merge impl that does nothing but copy data. All component types must be
/// cloneable and no type transformations are allowed
pub struct CopyCloneImpl<'a> {
    components: &'a HashMap<ComponentTypeId, ComponentRegistration>,
}

impl<'a> CopyCloneImpl<'a> {
    pub fn new(components: &'a HashMap<ComponentTypeId, ComponentRegistration>) -> Self {
        Self { components }
    }
}

impl<'a> legion::world::CloneImpl for CopyCloneImpl<'a> {
    fn map_component_type(
        &self,
        component_type: ComponentTypeId,
    ) -> (ComponentTypeId, ComponentMeta) {
        let comp_reg = &self.components[&component_type];
        (comp_reg.component_type_id(), comp_reg.meta().clone())
    }

    fn clone_components(
        &self,
        _src_world: &World,
        _src_component_storage: &ComponentStorage,
        _src_component_storage_indexes: Range<ComponentIndex>,
        src_type: ComponentTypeId,
        _src_entities: &[Entity],
        _dst_entities: &[Entity],
        src_data: *const u8,
        dst_data: *mut u8,
        num_components: usize,
    ) {
        let comp_reg = &self.components[&src_type];
        unsafe {
            comp_reg.clone_components(src_data, dst_data, num_components);
        }
    }
}

/// Trait for implementing clone merge mapping from one type to another
pub trait SpawnFrom<FromT: Sized>
where
    Self: Sized,
{
    fn spawn_from(
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: Range<ComponentIndex>,
        resources: &Resources,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        from: &[FromT],
        into: &mut [MaybeUninit<Self>],
    );
}

/// Trait for implementing clone merge mapping one type into another
pub trait SpawnInto<IntoT: Sized>
where
    Self: Sized,
{
    fn spawn_into(
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: Range<ComponentIndex>,
        resources: &Resources,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        from: &[Self],
        into: &mut [MaybeUninit<IntoT>],
    );
}

// From implies Into
impl<FromT, IntoT> SpawnInto<IntoT> for FromT
where
    IntoT: SpawnFrom<FromT>,
{
    fn spawn_into(
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: Range<ComponentIndex>,
        resources: &Resources,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        from: &[Self],
        into: &mut [MaybeUninit<IntoT>],
    ) {
        IntoT::spawn_from(
            src_world,
            src_component_storage,
            src_component_storage_indexes,
            resources,
            src_entities,
            dst_entities,
            from,
            into,
        );
    }
}

/// A registry of handlers for use with SpawnCloneImpl
pub struct SpawnCloneImplHandlerSet {
    handlers: HashMap<ComponentTypeId, Box<dyn SpawnCloneImplMapping>>,
}

impl SpawnCloneImplHandlerSet {
    /// Creates a new registry of handlers
    pub fn new() -> Self {
        Self {
            handlers: Default::default(),
        }
    }

    /// Adds a mapping from one component type to another. Rust's standard library into() will be
    /// used. This is a safe and idiomatic way to define mapping from one component type to another
    /// but has the downside of not providing access to the new world's resources
    pub fn add_mapping_into<FromT: Component + Clone + Into<IntoT>, IntoT: Component>(&mut self) {
        let from_type_id = ComponentTypeId::of::<FromT>();
        let into_type_id = ComponentTypeId::of::<IntoT>();
        let into_type_meta = ComponentMeta::of::<IntoT>();

        let handler = Box::new(SpawnCloneImplMappingImpl::new(
            into_type_id,
            into_type_meta,
            |_src_world,
             _src_component_storage,
             _src_component_storage_indexes,
             _resources,
             _src_entities,
             _dst_entities,
             src_data: *const u8,
             dst_data: *mut u8,
             num_components: usize| {
                unsafe {
                    let from_slice =
                        std::slice::from_raw_parts(src_data as *const FromT, num_components);
                    let to_slice = std::slice::from_raw_parts_mut(
                        dst_data as *mut MaybeUninit<IntoT>,
                        num_components,
                    );

                    from_slice.iter().zip(to_slice).for_each(|(from, to)| {
                        *to = MaybeUninit::new((*from).clone().into());
                    });
                }
            },
        ));

        self.handlers.insert(from_type_id, handler);
    }

    /// Adds a mapping from one component type to another. The trait impl will be passed the new
    /// world's resources and all the memory that holds the components. The memory passed into
    /// the closure as IntoT MUST be initialized or undefined behavior could happen on future access
    /// of the memory
    pub fn add_mapping<FromT: Component + Clone + SpawnInto<IntoT>, IntoT: Component>(&mut self) {
        let from_type_id = ComponentTypeId::of::<FromT>();
        let into_type_id = ComponentTypeId::of::<IntoT>();
        let into_type_meta = ComponentMeta::of::<IntoT>();

        let handler = Box::new(SpawnCloneImplMappingImpl::new(
            into_type_id,
            into_type_meta,
            |src_world,
             src_component_storage,
             src_component_storage_indexes,
             resources,
             src_entities,
             dst_entities,
             src_data: *const u8,
             dst_data: *mut u8,
             num_components: usize| {
                unsafe {
                    let from_slice =
                        std::slice::from_raw_parts(src_data as *const FromT, num_components);
                    let to_slice = std::slice::from_raw_parts_mut(
                        dst_data as *mut MaybeUninit<IntoT>,
                        num_components,
                    );

                    <FromT as SpawnInto<IntoT>>::spawn_into(
                        src_world,
                        src_component_storage,
                        src_component_storage_indexes,
                        resources,
                        src_entities,
                        dst_entities,
                        from_slice,
                        to_slice,
                    );
                }
            },
        ));

        self.handlers.insert(from_type_id, handler);
    }

    /// Adds a mapping from one component type to another. The closure will be passed the new
    /// world's resources and all the memory that holds the components. The memory passed into
    /// the closure as IntoT MUST be initialized or undefined behavior could happen on future access
    /// of the memory
    pub fn add_mapping_closure<FromT, IntoT, F>(
        &mut self,
        clone_fn: F,
    ) where
        FromT: Component,
        IntoT: Component,
        F: Fn(
                &World,                    // src_world
                &ComponentStorage,         // src_component_storage
                Range<ComponentIndex>,     // src_component_storage_indexes
                &Resources,                // resources
                &[Entity],                 // src_entities
                &[Entity],                 // dst_entities
                &[FromT],                  // src_data
                &mut [MaybeUninit<IntoT>], // dst_data
            ) + Send
            + Sync
            + 'static,
    {
        let from_type_id = ComponentTypeId::of::<FromT>();
        let into_type_id = ComponentTypeId::of::<IntoT>();
        let into_type_meta = ComponentMeta::of::<IntoT>();

        let handler = Box::new(SpawnCloneImplMappingImpl::new(
            into_type_id,
            into_type_meta,
            move |src_world,
                  src_component_storage,
                  src_component_storage_indexes,
                  resources,
                  src_entities,
                  dst_entities,
                  src_data: *const u8,
                  dst_data: *mut u8,
                  num_components: usize| {
                unsafe {
                    let from_slice =
                        std::slice::from_raw_parts(src_data as *const FromT, num_components);
                    let to_slice = std::slice::from_raw_parts_mut(
                        dst_data as *mut MaybeUninit<IntoT>,
                        num_components,
                    );
                    (clone_fn)(
                        src_world,
                        src_component_storage,
                        src_component_storage_indexes,
                        resources,
                        src_entities,
                        dst_entities,
                        from_slice,
                        to_slice,
                    );
                }
            },
        ));

        self.handlers.insert(from_type_id, handler);
    }
}

/// A CloneMergeImpl that
///
/// An implementation passed into legion::world::World::clone_merge. This implementation supports
/// providing custom mappings with add_mapping (which takes a closure) and add_mapping_into (which
/// uses Rust standard library's .into(). If a mapping isn't provided for a type, the component
/// will be cloned using ComponentRegistration passed in new()
pub struct SpawnCloneImpl<'a, 'b, 'c> {
    handler_set: &'a SpawnCloneImplHandlerSet,
    components: &'b HashMap<ComponentTypeId, ComponentRegistration>,
    resources: &'c Resources,
}

impl<'a, 'b, 'c> SpawnCloneImpl<'a, 'b, 'c> {
    /// Creates a new implementation
    pub fn new(
        handler_set: &'a SpawnCloneImplHandlerSet,
        components: &'b HashMap<ComponentTypeId, ComponentRegistration>,
        resources: &'c Resources,
    ) -> Self {
        Self {
            handler_set,
            components,
            resources,
        }
    }
}

impl<'a, 'b, 'c> legion::world::CloneImpl for SpawnCloneImpl<'a, 'b, 'c> {
    fn map_component_type(
        &self,
        component_type: ComponentTypeId,
    ) -> (ComponentTypeId, ComponentMeta) {
        // We expect any type we will encounter to be registered either as an explicit mapping or
        // registered in the component registrations
        let handler = &self.handler_set.handlers.get(&component_type);
        if let Some(handler) = handler {
            (handler.dst_type_id(), handler.dst_type_meta())
        } else {
            let comp_reg = &self.components[&component_type];
            (comp_reg.component_type_id(), comp_reg.meta().clone())
        }
    }

    fn clone_components(
        &self,
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: Range<ComponentIndex>,
        src_type: ComponentTypeId,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        src_data: *const u8,
        dst_data: *mut u8,
        num_components: usize,
    ) {
        // We expect any type we will encounter to be registered either as an explicit mapping or
        // registered in the component registrations
        let handler = &self.handler_set.handlers.get(&src_type);
        if let Some(handler) = handler {
            handler.clone_components(
                src_world,
                src_component_storage,
                src_component_storage_indexes,
                self.resources,
                src_entities,
                dst_entities,
                src_data,
                dst_data,
                num_components,
            );
        } else {
            let comp_reg = &self.components[&src_type];
            unsafe {
                comp_reg.clone_components(src_data, dst_data, num_components);
            }
        }
    }
}

/// Used internally to dynamic dispatch into a Box<CloneMergeMappingImpl<T>>
/// These are created as mappings are added to CloneMergeImpl
trait SpawnCloneImplMapping: Send + Sync {
    fn dst_type_id(&self) -> ComponentTypeId;
    fn dst_type_meta(&self) -> ComponentMeta;
    fn clone_components(
        &self,
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: Range<ComponentIndex>,
        resources: &Resources,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        src_data: *const u8,
        dst_data: *mut u8,
        num_components: usize,
    );
}

struct SpawnCloneImplMappingImpl<F>
where
    F: Fn(
        &World,                // src_world
        &ComponentStorage,     // src_component_storage
        Range<ComponentIndex>, // src_component_storage_indexes
        &Resources,            // resources
        &[Entity],             // src_entities
        &[Entity],             // dst_entities
        *const u8,             // src_data
        *mut u8,               // dst_data
        usize,                 // num_components
    ),
{
    dst_type_id: ComponentTypeId,
    dst_type_meta: ComponentMeta,
    clone_fn: F,
}

impl<F> SpawnCloneImplMappingImpl<F>
where
    F: Fn(
        &World,                // src_world
        &ComponentStorage,     // src_component_storage
        Range<ComponentIndex>, // src_component_storage_indexes
        &Resources,            // resources
        &[Entity],             // src_entities
        &[Entity],             // dst_entities
        *const u8,             // src_data
        *mut u8,               // dst_data
        usize,                 // num_components
    ),
{
    fn new(
        dst_type_id: ComponentTypeId,
        dst_type_meta: ComponentMeta,
        clone_fn: F,
    ) -> Self {
        SpawnCloneImplMappingImpl {
            dst_type_id,
            dst_type_meta,
            clone_fn,
        }
    }
}

impl<F> SpawnCloneImplMapping for SpawnCloneImplMappingImpl<F>
where
    F: Fn(
            &World,                // src_world
            &ComponentStorage,     // src_component_storage
            Range<ComponentIndex>, // src_component_storage_indexes
            &Resources,            // resources
            &[Entity],             // src_entities
            &[Entity],             // dst_entities
            *const u8,             // src_data
            *mut u8,               // dst_data
            usize,                 // num_components
        ) + Send
        + Sync,
{
    fn dst_type_id(&self) -> ComponentTypeId {
        self.dst_type_id
    }

    fn dst_type_meta(&self) -> ComponentMeta {
        self.dst_type_meta
    }

    fn clone_components(
        &self,
        src_world: &World,
        src_component_storage: &ComponentStorage,
        src_component_storage_indexes: Range<ComponentIndex>,
        resources: &Resources,
        src_entities: &[Entity],
        dst_entities: &[Entity],
        src_data: *const u8,
        dst_data: *mut u8,
        num_components: usize,
    ) {
        (self.clone_fn)(
            src_world,
            src_component_storage,
            src_component_storage_indexes,
            resources,
            src_entities,
            dst_entities,
            src_data,
            dst_data,
            num_components,
        );
    }
}
