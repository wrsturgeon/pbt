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
                    many_wow: 17528150796657311260,
                    such_amaze: 15480388469539559217,
                    pals: vec![],
                },
                Doggo::Subwoofer {
                    many_wow: 17832301479604389652,
                    such_amaze: 15501323363342400336,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 2490236031837522861,
                        such_amaze: 16429615213713786723,
                        pals: vec![],
                    }],
                },
                Doggo::Subwoofer {
                    many_wow: 3188832223763215729,
                    such_amaze: 3498615991784908799,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 15148293534712459978,
                        such_amaze: 18341947850468473462,
                        pals: vec![],
                    }],
                },
                Doggo::Subwoofer {
                    many_wow: 9269435554221061191,
                    such_amaze: 15297866277087607355,
                    pals: vec![],
                },
                Doggo::Subwoofer {
                    many_wow: 14291816033014134948,
                    such_amaze: 6355908936834482639,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 17777630582501953149,
                        such_amaze: 2698910952154039253,
                        pals: vec![],
                    }],
                },
                Doggo::Subwoofer {
                    many_wow: 11463920556086652082,
                    such_amaze: 1667746334847768353,
                    pals: vec![
                        Doggo::Subwoofer {
                            many_wow: 3601586158185195157,
                            such_amaze: 13656106926910895353,
                            pals: vec![],
                        },
                        Doggo::Subwoofer {
                            many_wow: 8535477529676816350,
                            such_amaze: 10628562347682653130,
                            pals: vec![],
                        },
                    ],
                },
                Doggo::Subwoofer {
                    many_wow: 5281299705294896765,
                    such_amaze: 1162275910445243082,
                    pals: vec![Doggo::Subwoofer {
                        many_wow: 7017724951860516012,
                        such_amaze: 9521738277975468457,
                        pals: vec![Doggo::Subwoofer {
                            many_wow: 1091359852592224527,
                            such_amaze: 11558433185276947975,
                            pals: vec![Doggo::Woofer],
                        }],
                    }],
                },
                Doggo::Subwoofer {
                    many_wow: 14395861734157766496,
                    such_amaze: 10829250029801242747,
                    pals: vec![],
                },
            ]
        );
    }
}
