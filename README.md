# `pbt`
### High-throughput property-based testing with `derive`, swarm-testing, precise sizing, and full graph-theoretic type analysis over mutually inductive and uninstantiable types.

This is a property-based testing library designed from the ground up with three goals:
1. Automate all boilerplate with `#[derive(Pbt)]` to mitigate human error.
2. Scale to huge values, mutually inductive types, and uninstantiable types correctly.
3. Shrink as close to a global minimum as possible without sacrificing efficiency.

`pbt` also enables [swarm testing](https://dl.acm.org/doi/pdf/10.1145/2338965.2336763) by default.

In short, this crate is mainly a just-in-time state machine compiler for generating algebraic data types,
and standard property-based testing algorithms (remarkably!) fall out as trivial graph operations.

## Quick example

```rust
#[pbt]
fn string_len_is_char_count(s: &String) {
    assert_eq!(s.len(), s.chars().count());
}
```

````text
$ cargo test
running 1 test
test string_len_is_char_count ... FAILED

failures:

---- string_len_is_char_count stdout ----

thread 'string_len_is_char_count' (522622) panicked at pbt-tests/src/lib.rs:57:1:

Consider the following input:

```
"\u{80}"
```

assertion `left == right` failed
  left: 2
 right: 1
````

See `./pbt-tests/src/lib.rs` to run this example and a few others.

## Witness persistence

Minimized witnesses are persisted under `.pbt` and replayed before newly generated inputs.
Set `PBT_CACHE_DIR` to choose another corpus directory, `PBT_NO_REPLAY` to bypass replay, or
`PBT_NO_PERSIST` to bypass writes. A flag is enabled by any nonempty value other than `0`.

Code that must avoid all persistence-related filesystem access can call
`pbt::witness_without_persistence`, which neither replays nor persists witnesses.

## Using `derive` on custom types

```rust
//                                    vvv
#[derive(Clone, Debug, Eq, PartialEq, Pbt)]
pub enum Foo {
    Bar,
    Baz { a: u64, b: u64, c: Vec<Foo> },
}

impl Foo {
    pub fn qux(&self) -> usize {
        match *self {
            Self::Bar => 0,
            Self::Baz { ref c, .. } => c.len(),
        }
    }
}

#[pbt]
fn search_and_minimize(foo: &Foo) {
    assert!(foo.qux() < 3);
}
```

````text
$ cargo test
running 1 test
test search_and_minimize ... FAILED

failures:

---- search_and_minimize stdout ----

thread 'search_and_minimize' (536547) panicked at pbt-tests/src/lib.rs:30:5:

Consider the following input:

```
Baz {
    a: 0,
    b: 0,
    c: [
        Bar,
        Bar,
        Bar,
    ],
}
```

assertion failed: foo.qux() < 3
````

See `./pbt-tests/src/lib.rs` to run this example and a few others.

## How does it work?

Standard property-based testing libraries work well in most cases,
but with unusual structures (e.g. a variant with multiple `Box<Self>` fields),
existing PBT libraries will either overflow their stack from multiplying their search space or
produce terms only from a hard-coded size distribution, missing very small and very large values.
Furthermore, uninstantiable types (e.g. `!` or `core::convert::Infallible`) will panic instead of
satisfying expected properties (e.g., a predicate with an argument of type `!` holds vacuously).

To avoid these pitfalls and reap the benefits of a simpler approach,
this library is built around a small, trusted core based on graph theory:
in short, this crate is mainly a just-in-time state machine compiler for generating algebraic data types,
and standard property-based testing algorithms (remarkably!) fall out as trivial graph operations.
Each type used in a `pbt` function is registered with a global type graph:
iff a type `T` has a constructor (e.g. an enum variant or an entire `struct`)
with a field of type `U`, then this graph contains a directed edge from `T` to `U`.
The notion of inductive types arises naturally: `T` is inductive iff there exists a cycle from `T` to `T`,
and each strongly connected component is a set of types defined via mutual induction.

Why is all of this useful? Let's say we want to generate a term of size 42.
Informally, we want to choose "big" (inductive) constructors until we're short on remaining size,
at which point we want "small" constructors which will let us out of a loop to a leaf value.
Dually, when we're shrinking a witness, we want to move from "big" constructors to "small" constructors
while reusing fields of our original constructor instead of generating new data.
The graph-theoretic approach solves both of these problems at once:
constructors are merely their index and a multiset of types (representing their fields).
A constructor is "big" iff any of its fields has a path back to the constructor's `Self` type
(since that constructor can use induction to build an arbitrarily large term),
and a constructor is "small" iff there exists a path to a leaf without visiting `Self`.
We precompute and cache these two sets of constructors so that
the generation process is as simple and efficient as possible:
pick a large constructor with some probability related to `size` (or else a small constructor),
partition that `size` across all inductive fields of the chosen constructor,
generate a multiset of terms with those sizes, and apply the constructor.
Dually, shrinking is simultaneously simpler and more powerful than any other PBT library to my knowledge:
first, visit all subtrees of a witness try them all as top-level witnesses,
then try all constructors whose fields are a sub(multi)set of the fields we already have,
and finally recurse by shrinking each field of the constructor.
Note that the first step (recursing to all subtrees with the same type as the top-level value)
is excruciatingly difficult to automate under normal circumstances,
but the choice of representation makes it almost trivial: recursively visit all fields in the multiset.
These choices also keep macro code minimal: all they do is declaratively register a type,
and the rest is logic defined in the core crate, as opposed to hard-coding generation or shrinking algorithms.
