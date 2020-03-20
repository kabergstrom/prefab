use core::ops::Range;
use legion::storage::ComponentStorage;
use legion::storage::ComponentTypeId;
use legion::storage::Component;
use legion::index::ComponentIndex;

/// Given an optional iterator, this will return Some(iter.next()) or Some(None) up to n times.
/// For a simpler interface for a slice/range use create_option_iter_from_slice, which will return
/// Some(&T) for each element in the range, or Some(None) for each element.
///
/// This iterator is intended for zipping an Option<Iter> with other Iters
pub struct OptionIter<T, U>
where
    T: std::iter::Iterator<Item = U>,
{
    opt: Option<T>,
    count: usize,
}

impl<T, U> OptionIter<T, U>
where
    T: std::iter::Iterator<Item = U>,
{
    fn new(
        opt: Option<T>,
        count: usize,
    ) -> Self {
        OptionIter::<T, U> { opt, count }
    }
}

impl<T, U> std::iter::Iterator for OptionIter<T, U>
where
    T: std::iter::Iterator<Item = U>,
{
    type Item = Option<U>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count <= 0 {
            return None;
        }

        self.count -= 1;
        self.opt
            .as_mut()
            .map_or_else(|| Some(None), |x| Some(x.next()))
    }
}

fn option_iter_from_slice<X>(
    opt: Option<&[X]>,
    range: Range<usize>,
) -> OptionIter<std::slice::Iter<X>, &X> {
    let mapped = opt.map(|x| (x[range.clone()]).iter());
    OptionIter::new(mapped, range.end - range.start)
}

fn get_components_in_storage<T: Component>(component_storage: &ComponentStorage) -> Option<&[T]> {
    unsafe {
        component_storage
            .components(ComponentTypeId::of::<T>())
            .map(|x| *x.data_slice::<T>())
    }
}

/// Given component storage and a range, this function will return an iterator of all components of
/// type T within the range.
///
/// When next is called on the iterator, one of three results can occur:
///  - None: The iterator has reached the end of the range
///  - Some(None): The entity does not have a component of type T attached to it
///  - Some(Some(T)): The entity has a component of type T attached to it
pub fn iter_components_in_storage<T: Component>(
    component_storage: &ComponentStorage,
    component_storage_indexes: Range<ComponentIndex>,
) -> OptionIter<core::slice::Iter<T>, &T> {
    let all_position_components = get_components_in_storage::<T>(component_storage);
    let range = component_storage_indexes.start.0..component_storage_indexes.end.0;
    option_iter_from_slice(all_position_components, range)
}
