#![cfg_attr(
    test,
    expect(clippy::tests_outside_test_module, reason = "This is a test module.")
)]

//! Tests for `pbt` as seen by downstream crates.

use pbt::{Pbt, pbt};

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

/// Reproduction of a prior failure that produced an "SCC missing" error.
#[derive(Clone, Debug, PartialEq, Pbt)]
pub struct SccRepro(Vec<(bool, usize)>);

#[pbt]
#[should_panic(
    expected = "\r\nConsider the following input:\r\n\r\n```\r\nVariable {\n    de_bruijn: 42,\n}\r\n```\r\n\r\nassertion failed: de_bruijn < 42"
)]
fn less_than_42(lc: &LambdaCalculus) {
    if let LambdaCalculus::Variable { de_bruijn } = *lc {
        assert!(de_bruijn < 42);
    }
}

#[pbt(1)]
fn scc_missing_repro(_: &SccRepro) {}

#[pbt]
#[should_panic(
    expected = "\r\nConsider the following input:\r\n\r\n```\r\n(\n    1,\n    0,\n)\r\n```\r\n\r\nassertion failed: lhs <= rhs"
)]
fn lhs_at_most_rhs(lhs: &usize, rhs: &usize) {
    assert!(lhs <= rhs);
}

#[pbt]
#[should_panic(
    expected = "\r\nConsider the following input:\r\n\r\n```\r\n\"\\u{80}\"\r\n```\r\n\r\nassertion `left == right` failed\n  left: 2\n right: 1"
)]
fn string_len_is_char_count(s: &String) {
    assert_eq!(s.len(), s.chars().count());
}
