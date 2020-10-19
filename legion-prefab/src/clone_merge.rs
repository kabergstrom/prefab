use std::collections::HashMap;
use crate::ComponentRegistration;
use legion::storage::{
    ComponentTypeId, Component, ComponentStorage, Components, EntityLayout, Archetype,
    ArchetypeWriter, ComponentWriter,
};
use legion::*;
use std::ops::Range;
use std::hash::BuildHasher;
use legion::world::{EntityRewrite, Allocate};
use std::marker::PhantomData;
use legion::world::EntityHasher;

/// A trivial clone merge impl that does nothing but copy data. All component types must be
/// cloneable and no type transformations are allowed
#[derive(Copy, Clone)]
pub struct CopyClone<'a, S: BuildHasher> {
    components: &'a HashMap<ComponentTypeId, ComponentRegistration, S>,
}

impl<'a, S: BuildHasher> CopyClone<'a, S> {
    pub fn new(components: &'a HashMap<ComponentTypeId, ComponentRegistration, S>) -> Self {
        Self { components }
    }
}

impl<'a, S: BuildHasher> legion::world::Merger for CopyClone<'a, S> {
    fn prefers_new_archetype() -> bool {
        false
    }

    fn convert_layout(
        &mut self,
        source_layout: EntityLayout,
    ) -> EntityLayout {
        let mut dest_layout = EntityLayout::default();
        for component_type in source_layout.component_types() {
            let comp_reg = &self.components[component_type];
            comp_reg.register_component(&mut dest_layout);
        }

        dest_layout
    }

    fn merge_archetype(
        &mut self,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ArchetypeWriter,
    ) {
        for src_type in src_arch.layout().component_types() {
            let comp_reg = &self.components[src_type];
            unsafe {
                comp_reg.clone_components(src_entity_range.clone(), src_arch, src_components, dst);
            }
        }
    }
}

/// Trait for implementing clone merge mapping from one type to another
pub trait SpawnFrom<FromT: Sized>
where
    Self: Sized + Component,
{
    #[allow(clippy::too_many_arguments)]
    fn spawn_from(
        resources: &Resources,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ComponentWriter<Self>,
        push_fn: fn(&mut ComponentWriter<Self>, Self),
    );
}

/// Trait for implementing clone merge mapping one type into another
pub trait SpawnInto<IntoT: Component>
where
    Self: Sized,
{
    #[allow(clippy::too_many_arguments)]
    fn spawn_into(
        resources: &Resources,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ComponentWriter<IntoT>,
        push_fn: fn(&mut ComponentWriter<IntoT>, IntoT),
    );
}

// From implies Into
impl<FromT, IntoT> SpawnInto<IntoT> for FromT
where
    IntoT: SpawnFrom<FromT> + Component,
{
    fn spawn_into(
        resources: &Resources,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ComponentWriter<IntoT>,
        push_fn: fn(&mut ComponentWriter<IntoT>, IntoT),
    ) {
        IntoT::spawn_from(
            resources,
            src_entity_range,
            src_arch,
            src_components,
            dst,
            push_fn,
        );
    }
}

/// A registry of handlers for use with SpawnClone
#[derive(Default)]
pub struct SpawnCloneHandlerSet {
    handlers: HashMap<ComponentTypeId, Box<dyn SpawnCloneMapping>>,
}

impl SpawnCloneHandlerSet {
    /// Creates a new registry of handlers
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mapping from one component type to another. Rust's standard library into() will be
    /// used. This is a safe and idiomatic way to define mapping from one component type to another
    /// but has the downside of not providing access to the new world's resources
    pub fn add_mapping_into<FromT: Component + Clone + Into<IntoT>, IntoT: Component>(&mut self) {
        let from_type_id = ComponentTypeId::of::<FromT>();
        let into_type_id = ComponentTypeId::of::<IntoT>();

        let handler = Box::new(SpawnCloneMappingImpl::<_, IntoT>::new(
            into_type_id,
            |_resources: &Resources,
             src_entity_range: Range<usize>,
             src_arch: &Archetype,
             src_components: &Components,
             dst: &mut ArchetypeWriter| {
                unsafe {
                    let src = src_components.get_downcast::<FromT>().unwrap();
                    let mut dst = dst.claim_components::<IntoT>();

                    let src_slice =
                        &src.get(src_arch.index()).unwrap().into_slice()[src_entity_range];
                    dst.ensure_capacity(src_slice.len());
                    for component in src_slice {
                        let cloned = <FromT as Clone>::clone(&component).into();
                        dst.extend_memcopy(&cloned as *const IntoT, 1);
                        std::mem::forget(cloned);
                    }
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

        let handler = Box::new(SpawnCloneMappingImpl::<_, IntoT>::new(
            into_type_id,
            |resources: &Resources,
             src_entity_range: Range<usize>,
             src_arch: &Archetype,
             src_components: &Components,
             dst: &mut ArchetypeWriter| {
                unsafe {
                    let src = src_components.get_downcast::<FromT>().unwrap();
                    let mut dst = dst.claim_components::<IntoT>();

                    let src_slice =
                        &src.get(src_arch.index()).unwrap().into_slice()[src_entity_range.clone()];
                    dst.ensure_capacity(src_slice.len());
                    <FromT as SpawnInto<IntoT>>::spawn_into(
                        resources,
                        src_entity_range,
                        src_arch,
                        src_components,
                        &mut dst,
                        |dst, into| {
                            dst.extend_memcopy(&into as *const IntoT, 1);
                            std::mem::forget(into);
                        },
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
                &Resources,                             // resources
                Range<usize>,                           // src_entity_range
                &Archetype,                             // src_arch
                &Components,                            // src_components
                &mut ComponentWriter<IntoT>,            // dst
                fn(&mut ComponentWriter<IntoT>, IntoT), // push_fn
            ) + Send
            + Sync
            + 'static,
    {
        let from_type_id = ComponentTypeId::of::<FromT>();
        let into_type_id = ComponentTypeId::of::<IntoT>();

        let handler = Box::new(SpawnCloneMappingImpl::<_, IntoT>::new(
            into_type_id,
            move |resources: &Resources,
                  src_entity_range: Range<usize>,
                  src_arch: &Archetype,
                  src_components: &Components,
                  dst: &mut ArchetypeWriter| {
                unsafe {
                    let src = src_components.get_downcast::<FromT>().unwrap();
                    let mut dst = dst.claim_components::<IntoT>();

                    let src_slice =
                        &src.get(src_arch.index()).unwrap().into_slice()[src_entity_range.clone()];
                    dst.ensure_capacity(src_slice.len());

                    (clone_fn)(
                        resources,
                        src_entity_range,
                        src_arch,
                        src_components,
                        &mut dst,
                        |dst, into| {
                            dst.extend_memcopy(&into as *const IntoT, 1);
                            std::mem::forget(into);
                        },
                    );
                }
            },
        ));

        self.handlers.insert(from_type_id, handler);
    }
}

/// An implementation passed into legion::world::World::clone_from. This implementation supports
/// providing custom mappings with add_mapping (which takes a closure) and add_mapping_into (which
/// uses Rust standard library's .into(). If a mapping isn't provided for a type, the component
/// will be cloned using ComponentRegistration passed in new()
pub struct SpawnClone<'a, 'b, 'c, 'd, S: BuildHasher> {
    handler_set: &'a SpawnCloneHandlerSet,
    components: &'b HashMap<ComponentTypeId, ComponentRegistration, S>,
    resources: &'c Resources,
    entity_map: &'d HashMap<Entity, Entity, EntityHasher>,
}

impl<'a, 'b, 'c, 'd, S: BuildHasher> SpawnClone<'a, 'b, 'c, 'd, S> {
    /// Creates a new implementation
    pub fn new(
        handler_set: &'a SpawnCloneHandlerSet,
        components: &'b HashMap<ComponentTypeId, ComponentRegistration, S>,
        resources: &'c Resources,
        entity_map: &'d HashMap<Entity, Entity, EntityHasher>,
    ) -> Self {
        Self {
            handler_set,
            components,
            resources,
            entity_map,
        }
    }
}

impl<'a, 'b, 'c, 'd, S: BuildHasher> legion::world::Merger for SpawnClone<'a, 'b, 'c, 'd, S> {
    fn prefers_new_archetype() -> bool {
        false
    }

    /// Indicates how the merger wishes entity IDs to be adjusted while cloning a world.
    fn entity_map(&mut self) -> EntityRewrite {
        EntityRewrite::default()
    }

    /// Returns the ID to use in the destination world when cloning the given entity.
    #[inline]
    #[allow(unused_variables)]
    fn assign_id(
        &mut self,
        existing: Entity,
        allocator: &mut Allocate,
    ) -> Entity {
        if let Some(e) = self.entity_map.get(&existing) {
            *e
        } else {
            allocator.next().unwrap()
        }
    }

    fn convert_layout(
        &mut self,
        source_layout: EntityLayout,
    ) -> EntityLayout {
        let mut dest_layout = EntityLayout::default();
        for component_type in source_layout.component_types() {
            // We expect any type we will encounter to be registered either as an explicit mapping or
            // registered in the component registrations
            let handler = &self.handler_set.handlers.get(&component_type);
            if let Some(handler) = handler {
                handler.register_dst_type(&mut dest_layout);
            } else {
                let comp_reg = &self.components[component_type];
                comp_reg.register_component(&mut dest_layout);
            }
        }

        dest_layout
    }

    fn merge_archetype(
        &mut self,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ArchetypeWriter,
    ) {
        for src_type in src_arch.layout().component_types() {
            // We expect any type we will encounter to be registered either as an explicit mapping or
            // registered in the component registrations
            let handler = &self.handler_set.handlers.get(&src_type);
            if let Some(handler) = handler {
                handler.clone_components(
                    self.resources,
                    src_entity_range.clone(),
                    src_arch,
                    src_components,
                    dst,
                )
            } else {
                let comp_reg = &self.components[&src_type];
                unsafe {
                    comp_reg.clone_components(
                        src_entity_range.clone(),
                        src_arch,
                        src_components,
                        dst,
                    );
                }
            }
        }
    }
}

/// Used internally to dynamic dispatch into a Box<SpawnCloneMapping>
/// These are created as mappings are added to SpawnClone
trait SpawnCloneMapping: Send + Sync {
    fn dst_type_id(&self) -> ComponentTypeId;

    fn register_dst_type(
        &self,
        entity_layout: &mut EntityLayout,
    );

    #[allow(clippy::too_many_arguments)]
    fn clone_components(
        &self,
        resources: &Resources,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ArchetypeWriter,
    );
}

struct SpawnCloneMappingImpl<F, IntoT>
where
    F: Fn(
        &Resources,           // resources
        Range<usize>,         // src_entity_range
        &Archetype,           // src_arch
        &Components,          // src_components
        &mut ArchetypeWriter, // dst
    ),
    IntoT: 'static,
{
    dst_type_id: ComponentTypeId,
    //dst_type_meta: ComponentMeta,
    clone_fn: F,
    phantom_data: PhantomData<IntoT>,
}

impl<F, IntoT> SpawnCloneMappingImpl<F, IntoT>
where
    F: Fn(
        &Resources,           // resources
        Range<usize>,         // src_entity_range
        &Archetype,           // src_arch
        &Components,          // src_components
        &mut ArchetypeWriter, // dst
    ),
    IntoT: 'static,
{
    fn new(
        dst_type_id: ComponentTypeId,
        clone_fn: F,
    ) -> Self {
        SpawnCloneMappingImpl {
            dst_type_id,
            clone_fn,
            phantom_data: Default::default(),
        }
    }
}

impl<F, IntoT> SpawnCloneMapping for SpawnCloneMappingImpl<F, IntoT>
where
    F: Fn(
            &Resources,           // resources
            Range<usize>,         // src_entity_range
            &Archetype,           // src_arch
            &Components,          // src_components
            &mut ArchetypeWriter, // dst
        ) + Send
        + Sync,
    IntoT: 'static + Send + Sync,
{
    fn dst_type_id(&self) -> ComponentTypeId {
        self.dst_type_id
    }

    fn register_dst_type(
        &self,
        entity_layout: &mut EntityLayout,
    ) {
        entity_layout.register_component::<IntoT>();
    }

    fn clone_components(
        &self,
        resources: &Resources,
        src_entity_range: Range<usize>,
        src_arch: &Archetype,
        src_components: &Components,
        dst: &mut ArchetypeWriter,
    ) {
        (self.clone_fn)(resources, src_entity_range, src_arch, src_components, dst);
    }
}
