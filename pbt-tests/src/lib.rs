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

#[cfg(test)]
mod test {
    use {
        super::*,
        pbt::{SEED, WyRand, construct::arbitrary, size::Size},
        pretty_assertions::assert_eq,
    };

    #[test]
    fn deterministic_generation() {
        let mut prng = WyRand::new(u64::from(SEED));
        assert_eq!(
            Size::expanding()
                .take(10)
                .filter_map(|size| arbitrary(&mut prng, size))
                .collect::<Vec<Doggo>>(),
            vec![
                Doggo::Woofer,
                Doggo::Woofer,
                Doggo::Subwoofer {
                    many_wow: 4,
                    such_amaze: 1,
                    pals: vec![],
                },
                Doggo::Subwoofer {
                    many_wow: 358,
                    such_amaze: 79,
                    pals: vec![Doggo::Woofer,],
                },
                Doggo::Subwoofer {
                    many_wow: 0,
                    such_amaze: 4,
                    pals: vec![],
                },
                Doggo::Woofer,
                Doggo::Subwoofer {
                    many_wow: 3,
                    such_amaze: 13823,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 0,
                        such_amaze: 0,
                        pals: vec![Doggo::Subwoofer {
                            many_wow: 3,
                            such_amaze: 15952,
                            pals: vec![],
                        },],
                    },],
                },
                Doggo::Subwoofer {
                    many_wow: 2116,
                    such_amaze: 1,
                    pals: vec![
                        Doggo::Woofer,
                        Doggo::Subwoofer {
                            many_wow: 12083733,
                            such_amaze: 0,
                            pals: vec![],
                        },
                    ],
                },
                Doggo::Subwoofer {
                    many_wow: 12,
                    such_amaze: 5,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 18446744073709551615,
                        such_amaze: 12,
                        pals: vec![Doggo::Woofer, Doggo::Woofer,],
                    },],
                },
                Doggo::Subwoofer {
                    many_wow: 6,
                    such_amaze: 0,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 10,
                        such_amaze: 945336233,
                        pals: vec![],
                    },],
                },
            ],
        );
    }
}
