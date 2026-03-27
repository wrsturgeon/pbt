#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "failing tests ought to panic"
)]

use {
    crate::{
        construct::{Construct as _, Prng, arbitrary},
        hash::empty_set,
        reflection::{Constructors, TermsOfVariousTypes, TypeInfo, type_of},
    },
    core::{
        any::{TypeId, type_name},
        iter,
    },
    pretty_assertions::assert_eq,
};

#[test]
fn info_bool() {
    type T = bool;
    let TypeInfo {
        ref constructors,
        ref dependencies,
        name,
        trivial,
    } = *T::info();
    assert_eq!(name, type_name::<T>());
    let Constructors::Literal { .. } = *constructors else {
        panic!("expected literal (non-algebraic) constructors but found {constructors:#?}")
    };
    assert_eq!(dependencies.ctor_idx, None);
    assert_eq!(
        dependencies.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        dependencies.id.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(dependencies.reachable, empty_set());
    assert_eq!(dependencies.unavoidable, Some(empty_set()));
    assert!(trivial);
}

#[test]
fn info_box_bool() {
    type T = Box<bool>;
    let TypeInfo {
        ref constructors,
        ref dependencies,
        name,
        trivial,
    } = *T::info();
    assert_eq!(name, type_name::<T>());
    let Constructors::Algebraic(ref constructors) = *constructors else {
        panic!("expected algebraic constructors but found {constructors:#?}")
    };
    assert_eq!(constructors.all_tagged.len(), 1);
    assert_eq!(constructors.guaranteed_leaves.len(), 1);
    assert_eq!(constructors.guaranteed_loops.len(), 0);
    assert_eq!(constructors.potential_leaves.len(), 1);
    assert_eq!(constructors.potential_loops.len(), 0);
    assert_eq!(dependencies.ctor_idx, None);
    assert_eq!(
        dependencies.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        dependencies.id.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(
        dependencies.reachable,
        iter::once(type_of::<bool>()).collect(),
    );
    assert_eq!(
        dependencies.unavoidable,
        Some(iter::once(type_of::<bool>()).collect()),
    );
    assert!(trivial);
}

#[test]
fn visit_deep_bool() {
    let t = true;
    let f = false;
    assert_eq!(t.visit_deep().collect::<Vec<&bool>>(), vec![&true]);
    assert_eq!(f.visit_deep().collect::<Vec<&bool>>(), vec![&false]);
    assert_eq!(t.visit_deep().collect::<Vec<&u64>>(), Vec::<&u64>::new());
    assert_eq!(f.visit_deep().collect::<Vec<&u64>>(), Vec::<&u64>::new());
}

#[test]
fn visit_deep_box_bool() {
    let t = Box::new(true);
    let f = Box::new(false);
    assert_eq!(
        t.visit_deep().collect::<Vec<&Box<bool>>>(),
        vec![&Box::new(true)],
    );
    assert_eq!(
        f.visit_deep().collect::<Vec<&Box<bool>>>(),
        vec![&Box::new(false)],
    );
    assert_eq!(t.visit_deep().collect::<Vec<&bool>>(), vec![&true]);
    assert_eq!(f.visit_deep().collect::<Vec<&bool>>(), vec![&false]);
    assert_eq!(t.visit_deep().collect::<Vec<&u64>>(), Vec::<&u64>::new());
    assert_eq!(f.visit_deep().collect::<Vec<&u64>>(), Vec::<&u64>::new());
}

#[test]
fn visit_shallow_bool() {
    let t = true;
    let f = false;
    assert_eq!(t.visit_shallow().collect::<Vec<&bool>>(), vec![&true]);
    assert_eq!(f.visit_shallow().collect::<Vec<&bool>>(), vec![&false]);
    assert_eq!(t.visit_shallow().collect::<Vec<&u64>>(), Vec::<&u64>::new());
    assert_eq!(f.visit_shallow().collect::<Vec<&u64>>(), Vec::<&u64>::new());
}

#[test]
fn visit_shallow_box_bool() {
    let t = Box::new(true);
    let f = Box::new(false);
    assert_eq!(
        t.visit_shallow().collect::<Vec<&Box<bool>>>(),
        vec![&Box::new(true)],
    );
    assert_eq!(
        f.visit_shallow().collect::<Vec<&Box<bool>>>(),
        vec![&Box::new(false)],
    );
    assert_eq!(t.visit_shallow().collect::<Vec<&bool>>(), vec![&true]);
    assert_eq!(f.visit_shallow().collect::<Vec<&bool>>(), vec![&false]);
    assert_eq!(t.visit_shallow().collect::<Vec<&u64>>(), Vec::<&u64>::new());
    assert_eq!(f.visit_shallow().collect::<Vec<&u64>>(), Vec::<&u64>::new());
}

#[test]
fn terms_of_various_types() {
    let mut terms = TermsOfVariousTypes::new();
    let () = terms.push(42_u64);
    let () = terms.push(true);
    let () = terms.push(false);
    assert_eq!(terms.pop(), Some(false));
    assert_eq!(terms.pop(), Option::<Box<bool>>::None);
    assert_eq!(terms.pop(), Some(42_u64));
    assert_eq!(terms.pop(), Option::<u64>::None);
    // leave `true` intact to test that `Drop` doesn't leak:
}

#[test]
fn arbitrary_bool() {
    let mut prng = Prng::new(None);
    assert_eq!(
        iter::repeat_with(|| arbitrary(&mut prng).unwrap())
            .take(10)
            .collect::<Vec<bool>>(),
        vec![
            false, false, false, true, false, true, false, true, true, false,
        ],
    );
}

#[test]
fn arbitrary_u64() {
    let mut prng = Prng::new(None);
    assert_eq!(
        iter::repeat_with(|| arbitrary(&mut prng).unwrap())
            .take(10)
            .collect::<Vec<u64>>(),
        vec![
            6_502_630_866_907_404_834,
            18_353_055_445_182_403_062,
            10_599_744_798_405_285_088,
            974_438_577_008_164_883,
            366_399_349_974_675_270,
            15_480_388_469_539_559_217,
            17_528_150_796_657_311_260,
            14_774_801_171_373_612_679,
            9_171_889_233_211_178_199,
            15_721_880_942_338_967_310,
        ],
    );
}

#[test]
fn arbitrary_box_bool() {
    let mut prng = Prng::new(None);
    assert_eq!(
        iter::repeat_with(|| arbitrary(&mut prng).unwrap())
            .take(10)
            .collect::<Vec<Box<bool>>>(),
        [
            false, true, true, true, false, false, true, true, true, false,
        ]
        .into_iter()
        .map(Box::new)
        .collect::<Vec<_>>(),
    );
}
