#![expect(clippy::panic, reason = "failing tests ought to panic")]

use {
    crate::{
        construct::{Construct as _, arbitrary, check_beta_reduction, check_eta_expansion},
        hash::{SEED, empty_set},
        reflection::{
            PrecomputedTypeFormer, TermsOfVariousTypes, TypeInfo, breadth_first_transpose, info,
            type_of,
        },
        shrink::shrink,
        size::Size,
    },
    core::{
        any::{TypeId, type_name},
        iter,
    },
    pretty_assertions::assert_eq,
    wyrand::WyRand,
};

#[test]
fn info_bool() {
    type T = bool;
    let TypeInfo {
        ref type_former,
        ref dependencies,
        name,
        trivial,
    } = *info::<T>();
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Literal { .. } = *type_former else {
        panic!("expected literal (non-algebraic) type former but found {type_former:#?}")
    };
    assert_eq!(dependencies.constructor, None);
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
        ref type_former,
        ref dependencies,
        name,
        trivial,
    } = *info::<T>();
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(constructors.all_tagged.len(), 1);
    assert_eq!(constructors.guaranteed_leaves.len(), 1);
    assert_eq!(constructors.guaranteed_loops.len(), 0);
    assert_eq!(constructors.potential_leaves.len(), 1);
    assert_eq!(constructors.potential_loops.len(), 0);
    assert_eq!(dependencies.constructor, None);
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
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<bool>>(),
        vec![
            false, false, false, true, false, true, false, true, true, false,
        ],
    );
}

#[test]
fn arbitrary_u64() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
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
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<Box<bool>>>(),
        [
            false, false, true, true, true, true, false, false, true, true,
        ]
        .into_iter()
        .map(Box::new)
        .collect::<Vec<_>>(),
    );
}

#[test]
fn beta_reduction_bool() {
    let () = check_beta_reduction::<bool>(&mut WyRand::new(u64::from(SEED)));
}

#[test]
fn beta_reduction_box_bool() {
    let () = check_beta_reduction::<Box<bool>>(&mut WyRand::new(u64::from(SEED)));
}

#[test]
fn eta_expansion_bool() {
    let () = check_eta_expansion::<bool>(&mut WyRand::new(u64::from(SEED)));
}

#[test]
fn eta_expansion_box_bool() {
    let () = check_eta_expansion::<Box<bool>>(&mut WyRand::new(u64::from(SEED)));
}

#[test]
fn breadth_first_iteration() {
    let values: Vec<u64> = vec![10, 20, 40];
    let iterators = values.into_iter().map(|u| (u, shrink(u))).collect();
    let iterator = breadth_first_transpose(iterators);
    let iterated: Vec<Vec<u64>> = iterator.collect();
    ::std::assert_eq!(
        iterated,
        vec![
            vec![0, 20, 40],
            vec![10, 0, 40],
            vec![10, 20, 0],
            vec![5, 20, 40],
            vec![10, 10, 40],
            vec![10, 20, 20],
            vec![8, 20, 40],
            vec![10, 15, 40],
            vec![10, 20, 30],
            vec![9, 20, 40],
            vec![10, 18, 40],
            vec![10, 20, 35],
            vec![10, 19, 40],
            vec![10, 20, 38],
            vec![10, 20, 39],
        ],
    );
}

#[test]
fn shrink_bool() {
    assert_eq!(shrink(false).collect::<Vec<_>>(), vec![]);
    assert_eq!(shrink(true).collect::<Vec<_>>(), vec![false]);
}

#[test]
fn shrink_u64() {
    assert_eq!(
        shrink(100_u64).collect::<Vec<_>>(),
        vec![0, 50, 75, 88, 94, 97, 99],
    );
}

#[test]
fn shrink_box_u64() {
    assert_eq!(
        shrink(Box::new(100_u64)).collect::<Vec<_>>(),
        [0, 50, 75, 88, 94, 97, 99]
            .into_iter()
            .map(Box::new)
            .collect::<Vec<_>>(),
    );
}
