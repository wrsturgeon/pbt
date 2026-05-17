use {
    core::convert::Infallible,
    pbt::{
        Pbt,
        sigma::{Predicate, Sigma},
    },
};

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Pbt)]
pub enum Foo {
    Bar,
    Baz { a: u64, b: u64, c: Vec<Foo> },
}

#[derive(Clone, Debug, Eq, PartialEq, Pbt)]
pub enum PartiallyInstantiable {
    Instantiable,
    Uninstantiable(Infallible),
}

#[derive(Clone, Debug, Eq, PartialEq, Pbt)]
pub enum Uninhabited {}

pub type NonAnswer = Sigma<u8, NotTheAnswer>;

pub enum NotTheAnswer {}

impl Foo {
    #[inline]
    #[must_use]
    pub fn bus_factor(&self) -> usize {
        match *self {
            Self::Bar => 0,
            Self::Baz { ref c, .. } => c.len(),
        }
    }
}

impl Predicate<u8> for NotTheAnswer {
    type Error = String;

    #[inline]
    fn check(candidate: &u8) -> Result<(), Self::Error> {
        if *candidate == 42 {
            Err(format!(
                "The Answer to the Ultimate Question of Life, the Universe, and Everything is {candidate}",
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, pbt::search, pretty_assertions::assert_eq};

    const N_CASES: usize = 1_000;

    #[derive(Clone, Debug, Eq, PartialEq, Pbt)]
    enum ShapedNever {}

    #[derive(Clone, Debug, Eq, PartialEq, Pbt)]
    enum ShapedPayload<T> {
        Empty,
        Bits(Vec<T>),
    }

    #[derive(Clone, Debug, Eq, PartialEq, Pbt)]
    enum ShapedTree {
        Leaf,
        Branch {
            left: Box<Self>,
            value: usize,
            flags: Vec<bool>,
            right: Box<Self>,
        },
    }

    #[test]
    fn instantiability_logic() {
        search::assert_eq(N_CASES, |pi: &PartiallyInstantiable| {
            (pi.clone(), PartiallyInstantiable::Instantiable)
        });
    }

    #[test]
    fn search_and_minimize() {
        let maybe_witness: Option<Foo> =
            search::witness(N_CASES, |foo: &Foo| foo.bus_factor() >= 3);
        assert_eq!(
            maybe_witness,
            Some(Foo::Baz {
                a: 0,
                b: 0,
                c: vec![Foo::Bar, Foo::Bar, Foo::Bar],
            }),
        );
    }

    #[test]
    fn sigma() {
        search::assert(N_CASES, |u: &NonAnswer| **u != 42);
    }

    #[test]
    fn empty_enum_is_supported() {
        let maybe_witness: Option<Uninhabited> = search::witness(N_CASES, |_| true);
        assert_eq!(maybe_witness, None);
    }

    #[test]
    fn shaped_empty_enums_have_vacuous_eliminators() {
        let absurd: fn(&ShapedNever) -> usize = |never| never.elim(());
        let _ = absurd;
    }

    #[test]
    fn shaped_generic_fields_do_not_need_recursion() {
        let empty = <ShapedPayload<bool> as ShapedPayloadShaped>::empty(ShapedPayloadEmpty);
        let empty_len = empty.elim(
            (),
            |(), ShapedPayloadEmpty| 0_usize,
            |(), ShapedPayloadBits(bits)| bits.len(),
        );
        let payload =
            <ShapedPayload<bool> as ShapedPayloadShaped>::bits(ShapedPayloadBits(vec![true]));
        let len = payload.elim(
            (),
            |(), ShapedPayloadEmpty| 0_usize,
            |(), ShapedPayloadBits(bits)| bits.len(),
        );

        assert_eq!(empty_len, 0);
        assert_eq!(len, 1);
    }

    #[test]
    fn shaped_macro_generates_field_wise_eliminator() {
        let tree = <ShapedTree as ShapedTreeShaped>::branch(ShapedTreeBranch {
            left: Box::new(ShapedTree::Leaf),
            value: 7,
            flags: vec![true, false],
            right: Box::new(ShapedTree::Leaf),
        });

        let selected = tree.elim(
            5,
            |_, ShapedTreeLeaf| 0,
            |state, branch| {
                assert!(matches!(**branch.left, ShapedTree::Leaf));
                assert!(matches!(**branch.right, ShapedTree::Leaf));
                assert_eq!(branch.flags.as_slice(), &[true, false]);
                state + *branch.value
            },
        );

        assert_eq!(selected, 12);
    }
}
