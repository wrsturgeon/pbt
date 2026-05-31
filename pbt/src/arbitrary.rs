//! Generate an arbitrary term of any type `T`.

use {
    crate::{
        Pbt,
        hash::map,
        persist,
        reflection::{Uninstantiable, register_globally},
        size::Size,
        swarm::Swarm,
    },
    wyrand::WyRand,
};

/// Generate an arbitrary term of any type `T`.
///
/// # Errors
///
/// If `T` is uninstantiable.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn arbitrary<T>(prng: &mut WyRand) -> Result<impl Iterator<Item = T>, Uninstantiable>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    let mut swarm_cache = map();
    let mut swarm = Swarm::new::<T>(prng, &mut swarm_cache)?;
    let mut batch_size = 1_usize; // Increases over time.
    let mut remaining_in_batch = batch_size;
    Ok(persist::replay().chain(Size::increasing().map(move |size| {
        if let Some(decremented) = remaining_in_batch.checked_sub(1) {
            remaining_in_batch = decremented;
        } else {
            remaining_in_batch = batch_size;
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "The hardware will die before batch size overflows."
            )]
            let () = batch_size += 1;
            swarm = Swarm::new::<T>(prng, &mut swarm_cache)
                .expect("INTERNAL ERROR (`pbt`): instantiability changed mid-generation");
        }
        swarm.arbitrary(size, prng)
    })))
}
