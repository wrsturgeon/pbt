use pbt::Pbt;

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Pbt)]
pub enum Doggo {
    Woofer,
    Subwoofer {
        many_wow: u64,
        such_amaze: u64,
        pals: Vec<Doggo>,
    },
}

impl Doggo {
    #[inline]
    #[must_use]
    pub fn n_pals(&self) -> usize {
        match *self {
            Self::Woofer => 0, // :(
            Self::Subwoofer { ref pals, .. } => pals.len(),
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, pbt::search, pretty_assertions::assert_eq};

    #[test]
    fn search_and_minimize() {
        let popular_doggo: Option<Doggo> =
            search::witness(1_000, |doggo: &Doggo| doggo.n_pals() >= 3);
        assert_eq!(
            popular_doggo,
            Some(Doggo::Subwoofer {
                many_wow: 0,
                such_amaze: 0,
                pals: vec![Doggo::Woofer, Doggo::Woofer, Doggo::Woofer]
            }),
        );
    }
}
