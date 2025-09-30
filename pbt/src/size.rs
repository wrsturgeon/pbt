#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeInstantiable<Instantiable> {
    Instantiable(Instantiable),
    Uninstantiable,
}

#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeInfinite<Finite> {
    Finite(Finite),
    Infinite,
}

#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeOverflow<Contained> {
    Contained(Contained),
    Overflow,
}
