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

/// Generate arbitrary terms without a replay prefix.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
fn generate<T>(prng: &mut WyRand) -> Result<impl Iterator<Item = T>, Uninstantiable>
where
    T: Pbt,
{
    let mut swarm_cache = map();
    let mut swarm = Swarm::new::<T>(prng, &mut swarm_cache)?;
    let mut batch_size = 1_usize; // Increases over time.
    let mut remaining_in_batch = batch_size;
    Ok(Size::increasing().map(move |size| {
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
    }))
}

/// Generate an arbitrary term of any type `T`.
///
/// # Errors
///
/// If `T` is uninstantiable.
#[inline]
pub(crate) fn arbitrary<T>(prng: &mut WyRand) -> Result<impl Iterator<Item = T>, Uninstantiable>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    let generated = generate(prng)?;
    Ok(persist::replay().chain(generated))
}

/// Generate arbitrary terms without replaying persisted witnesses.
///
/// # Errors
///
/// If `T` is uninstantiable.
#[inline]
pub(crate) fn arbitrary_without_replay<T>(
    prng: &mut WyRand,
) -> Result<impl Iterator<Item = T>, Uninstantiable>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    generate(prng)
}
