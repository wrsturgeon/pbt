use {
    crate::{ast_size::AstSize, error},
    core::fmt,
    rand_core::{RngCore, SeedableRng},
};

// Supposedly higher throughput than a simple 64-bit multiply-and-add,
// plus much, much better qualiy than that, and
// coming from people who know what they're doing.
// Caution: low complexity in lower bits.
// `Xoshiro256**` is an alternative (~15% slowdown).
pub type DefaultRng = rand_xoshiro::Xoshiro256Plus;

/// Generate a pseudorandom instance of this type,
/// exhaustively covering all possible values in the limit,
/// with a precise statistical expectation of an AST size.
pub trait Pseudorandom: AstSize + fmt::Debug + Sized {
    /// Generate a pseudorandom instance of this type,
    /// exhaustively covering all possible values in the limit,
    /// with a precise statistically expected AST size.
    ///
    /// # Errors
    /// If and only if this type is uninstantiable.
    ///
    /// Q: Why not make instantiability a type-level distinction,
    /// e.g. panicking and reminding the user to check `Max`?
    /// A: Because sigma-types' instantiability
    /// won't be known in general until runtime.
    fn pseudorandom<Rng: RngCore>(
        expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable>;
}

/// Gnerate pseudorandom values of
/// increasingly large expected AST size,
/// starting from zero and increasing by one per item.
///
/// Note that this returns an effectively endless iterator,
/// so if you would like a finite number of values,
/// please use `.take(N)`.
#[inline]
pub fn pseudorandom<P: Pseudorandom, Rng: RngCore>(rng: &mut Rng) -> impl Iterator<Item = P> {
    (0..).map_while(|expected_ast_size: usize| {
        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            reason = "not meant to be precise"
        )]
        let expected_ast_size = expected_ast_size as f32;
        <P as Pseudorandom>::pseudorandom(expected_ast_size, rng).ok()
    })
}

#[inline]
#[must_use]
pub fn default_rng() -> DefaultRng {
    <DefaultRng as SeedableRng>::seed_from_u64(42)
}
