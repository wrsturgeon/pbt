#![allow(
    clippy::missing_panics_doc,
    clippy::tests_outside_test_module,
    reason = "testing-only module"
)]

use {core::cmp, pbt::Pbt};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Pbt)]
pub enum Void {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Pbt)]
pub enum Peano {
    O,
    S(Box<Self>),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Pbt)]
pub struct Pair(Peano, Peano);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Pbt)]
pub struct Wrapper<T>(T);

impl PartialEq<usize> for Peano {
    #[inline]
    fn eq(&self, other: &usize) -> bool {
        match *self {
            Self::O => matches!(other, 0),
            Self::S(ref pred) => {
                if let Some(other_pred) = other.checked_sub(1) {
                    <Self as PartialEq<usize>>::eq(pred, &other_pred)
                } else {
                    false
                }
            }
        }
    }
}

impl PartialOrd<usize> for Peano {
    #[inline]
    fn partial_cmp(&self, other: &usize) -> Option<cmp::Ordering> {
        match *self {
            Self::O => Some(if matches!(other, 0) {
                cmp::Ordering::Equal
            } else {
                cmp::Ordering::Less
            }),
            Self::S(ref pred) => {
                if let Some(other_pred) = other.checked_sub(1) {
                    <Self as PartialOrd<usize>>::partial_cmp(pred, &other_pred)
                } else {
                    Some(cmp::Ordering::Greater)
                }
            }
        }
    }
}

#[test]
fn unit_corners() {
    let corners: Vec<()> = <() as ::pbt::conjure::Conjure>::corners().collect();
    assert_eq!(corners, vec![()]);
}

#[test]
fn void() {
    let result = pbt::witness(|v: &Void| {
        dbg!(&v);
        true
    });
    pretty_assertions::assert_eq!(result, Err(pbt::NotFound));
}

#[test]
fn ge_ten() {
    let witness = pbt::witness(|p: &Peano| {
        dbg!(&p);
        *p >= 10
    });
    let ten = {
        let mut acc = Peano::O;
        for _ in 0..10 {
            acc = Peano::S(Box::new(acc));
        }
        acc
    };
    pretty_assertions::assert_eq!(witness, Ok(ten));
}

#[test]
fn ge_42() {
    let witness = pbt::witness(|p: &Peano| {
        dbg!(&p);
        *p >= 42
    });
    let forty_two = {
        let mut acc = Peano::O;
        for _ in 0..42 {
            acc = Peano::S(Box::new(acc));
        }
        acc
    };
    pretty_assertions::assert_eq!(witness, Ok(forty_two));
}

#[test]
fn wrapper_ge_42() {
    let witness = pbt::witness(|p: &Wrapper<Peano>| {
        dbg!(&p);
        p.0 >= 42
    });
    let forty_two = {
        let mut acc = Peano::O;
        for _ in 0..42 {
            acc = Peano::S(Box::new(acc));
        }
        Wrapper(acc)
    };
    pretty_assertions::assert_eq!(witness, Ok(forty_two));
}

#[test]
fn ordered_pair() {
    let counterexample = pbt::witness(|p: &Pair| {
        dbg!(&p);
        p.0 > p.1
    });
    let minimal = Pair(Peano::S(Box::new(Peano::O)), Peano::O);
    pretty_assertions::assert_eq!(counterexample, Ok(minimal));
}
