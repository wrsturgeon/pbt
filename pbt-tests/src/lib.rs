use {core::convert::Infallible, pbt::Pbt};

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

#[cfg(test)]
mod test {
    use {super::*, pbt::search, pretty_assertions::assert_eq};

    #[test]
    fn instantiability_logic() {
        search::assert_eq(1_000, |pi: &PartiallyInstantiable| {
            (pi.clone(), PartiallyInstantiable::Instantiable)
        });
    }

    #[test]
    fn search_and_minimize() {
        let maybe_witness: Option<Foo> = search::witness(1_000, |foo: &Foo| foo.bus_factor() >= 3);
        assert_eq!(
            maybe_witness,
            Some(Foo::Baz {
                a: 0,
                b: 0,
                c: vec![Foo::Bar, Foo::Bar, Foo::Bar],
            }),
        );
    }
}
