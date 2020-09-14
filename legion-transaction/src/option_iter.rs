use core::ops::Range;
use legion::storage::{ComponentStorage, ComponentSlice};
use legion::storage::Component;

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
        if self.count == 0 {
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

pub fn get_component_slice_from_archetype<'a, T: Component>(
    component_storage: &'a legion::storage::Components,
    src_arch: &legion::storage::Archetype,
    component_range: Range<usize>,
) -> Option<&'a [T]> {
    let component_storage: Option<&T::Storage> = component_storage.get_downcast::<T>();
    let slice: Option<ComponentSlice<'a, T>> =
        component_storage.map(|x| x.get(src_arch.index())).flatten();
    slice.map(|x| &x.into_slice()[component_range])
}

pub fn iter_component_slice_from_archetype<'a, T: Component>(
    component_storage: &'a legion::storage::Components,
    src_arch: &legion::storage::Archetype,
    component_range: Range<usize>,
) -> OptionIter<core::slice::Iter<'a, T>, &'a T> {
    let components =
        get_component_slice_from_archetype(component_storage, src_arch, component_range.clone());
    option_iter_from_slice(components, component_range)
}
