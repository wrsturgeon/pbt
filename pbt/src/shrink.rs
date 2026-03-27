use crate::{construct::Construct, reflection::TypeInfo};

#[inline]
pub fn shrink<T: Construct>(t: &T) -> impl Iterator<Item = T> {
    let TypeInfo { .. } = *T::info();

    // TODO:
    //   1. Visit all terms of type `Self` (deeply)
    //      and try them all as toplevel solutions.
    //   2. Try all constructors <= this one (TODO: by `Ord`)
    //      whose field multisets are subsets of `t`'s fields.
    //      TODO: should we do this for *all* (deep) terms
    //      or just immediate toplevel fields?
    //   3. Shrink fields within this constructor.
    //      TODO: should this be unified with the above,
    //      e.g. visiting all deep fields?
}
