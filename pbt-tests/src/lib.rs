//! Tests for `pbt` as seen by downstream crates.

//! End-to-end tests of the public `pbt` API.

use pbt::Pbt;

/// The lambda calculus with de Bruijn indices.
#[derive(Clone, Debug, PartialEq, Pbt)]
#[expect(
    clippy::exhaustive_enums,
    reason = "This test crate intentionally exposes a complete toy data type."
)]
pub enum LambdaCalculus {
    /// An application of one term to another
    /// (e.g. acting as a function applied to an argument).
    Application(Box<Self>, Box<Self>),
    /// A lambda-abstraction (e.g. acting as a closure).
    Lambda {
        /// The body scoped by this lambda-abstraction.
        body: Box<Self>,
    },
    /// A variable, identified by its de Bruijn index.
    Variable {
        /// How many binders to cross before finding this variable's binder.
        de_bruijn: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[pbt] // TODO: `#[pbt(1_000)]`
    #[should_panic(expected = "TODO: fill this in")]
    fn less_than_42(lc: &LambdaCalculus) {
        if let LambdaCalculus::Variable { de_bruijn } = *lc {
            assert!(de_bruijn < 42)
        }
    }
}
