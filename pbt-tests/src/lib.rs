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
}
