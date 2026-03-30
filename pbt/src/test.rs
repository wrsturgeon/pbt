#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "failing tests ought to panic"
)]

use {
    crate::{
        SEED,
        construct::{Construct as _, arbitrary, check_eta_expansion},
        reflection::{
            PrecomputedTypeFormer, TermsOfVariousTypes, TypeInfo, breadth_first_transpose, info,
            type_of,
        },
        search::witness,
        shrink::shrink,
        size::Size,
    },
    core::{
        any::{TypeId, type_name},
        convert::Infallible,
        iter,
    },
    pretty_assertions::assert_eq,
    std::collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    wyrand::WyRand,
};

#[test]
fn info_bool() {
    type T = bool;
    let info = info::<T>();
    let TypeInfo {
        name,
        trivial,
        ref type_former,
        ref vertex,
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Literal { .. } = *type_former else {
        panic!("expected literal (non-algebraic) type former but found {type_former:#?}")
    };
    assert_eq!(
        vertex.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.id.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(vertex.reachable, BTreeSet::new());
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert!(trivial);
    assert!(!vertex.inductive);
    assert!(info.instantiable());
}

#[test]
fn info_box_bool() {
    type T = Box<bool>;
    let info = info::<T>();
    let TypeInfo {
        name,
        trivial,
        ref type_former,
        ref vertex,
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(constructors.all_constructors.len(), 1);
    assert_eq!(constructors.guaranteed_leaves().len(), 1);
    assert_eq!(constructors.guaranteed_loops().len(), 0);
    assert_eq!(constructors.potential_leaves().len(), 1);
    assert_eq!(constructors.potential_loops().len(), 0);
    assert_eq!(
        vertex.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.id.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(vertex.reachable, iter::once(type_of::<bool>()).collect(),);
    assert_eq!(vertex.unavoidable, iter::once(type_of::<bool>()).collect(),);
    assert!(trivial);
    assert!(!vertex.inductive);
    assert!(info.instantiable());
}

#[test]
fn info_option_u64() {
    type T = Option<u64>;
    let info = info::<T>();
    let TypeInfo {
        name,
        trivial,
        ref type_former,
        ref vertex,
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(constructors.all_constructors.len(), 2);
    assert_eq!(constructors.guaranteed_leaves().len(), 2);
    assert_eq!(constructors.guaranteed_loops().len(), 0);
    assert_eq!(constructors.potential_leaves().len(), 2);
    assert_eq!(constructors.potential_loops().len(), 0);
    assert_eq!(
        vertex.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.id.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(vertex.reachable, iter::once(type_of::<u64>()).collect(),);
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert!(!trivial);
    assert!(!vertex.inductive);
    assert!(info.instantiable());
}

#[test]
fn info_vec_u64() {
    type T = Vec<u64>;
    let info = info::<T>();
    let TypeInfo {
        name,
        trivial,
        ref type_former,
        ref vertex,
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(
        vertex.reachable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert_eq!(constructors.all_constructors.len(), 2);
    assert_eq!(constructors.all_constructors[0].1.inductive, false);
    assert_eq!(
        constructors.all_constructors[0].1.unavoidable,
        BTreeSet::new(),
    );
    assert_eq!(
        constructors.all_constructors[0].1.reachable,
        BTreeSet::new()
    );
    assert_eq!(constructors.all_constructors[1].1.inductive, true);
    assert_eq!(
        constructors.all_constructors[1].1.unavoidable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(
        constructors.all_constructors[1].1.reachable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(constructors.guaranteed_leaves().len(), 1);
    assert_eq!(constructors.guaranteed_loops().len(), 1);
    assert_eq!(constructors.potential_leaves().len(), 1);
    assert_eq!(constructors.potential_loops().len(), 1);
    assert_eq!(
        vertex.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.id.id(),
        TypeId::of::<T>(),
    );
    assert!(!trivial);
    assert!(vertex.inductive);
    assert!(info.instantiable());
}

#[test]
fn info_btree_set_u64() {
    type T = BTreeSet<u64>;
    let info = info::<T>();
    let TypeInfo {
        name,
        trivial,
        ref type_former,
        ref vertex,
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(
        vertex.reachable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert_eq!(constructors.all_constructors.len(), 2);
    assert_eq!(constructors.all_constructors[0].1.inductive, false);
    assert_eq!(
        constructors.all_constructors[0].1.unavoidable,
        BTreeSet::new(),
    );
    assert_eq!(
        constructors.all_constructors[0].1.reachable,
        BTreeSet::new()
    );
    assert_eq!(constructors.all_constructors[1].1.inductive, true);
    assert_eq!(
        constructors.all_constructors[1].1.unavoidable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(
        constructors.all_constructors[1].1.reachable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(constructors.guaranteed_leaves().len(), 1);
    assert_eq!(constructors.guaranteed_loops().len(), 1);
    assert_eq!(constructors.potential_leaves().len(), 1);
    assert_eq!(constructors.potential_loops().len(), 1);
    assert_eq!(
        vertex.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.id.id(),
        TypeId::of::<T>(),
    );
    assert!(!trivial);
    assert!(vertex.inductive);
    assert!(info.instantiable());
}

#[test]
fn info_infallible() {
    type T = Infallible;
    let info = info::<T>();
    let TypeInfo {
        name,
        trivial,
        ref type_former,
        ref vertex,
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(vertex.reachable, BTreeSet::new());
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert!(
        constructors.all_constructors.is_empty(),
        "{:#?}",
        constructors.all_constructors,
    );
    assert_eq!(constructors.guaranteed_leaves().len(), 0);
    assert_eq!(constructors.guaranteed_loops().len(), 0);
    assert_eq!(constructors.potential_leaves().len(), 0);
    assert_eq!(constructors.potential_loops().len(), 0);
    assert_eq!(
        vertex.id,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.id.id(),
        TypeId::of::<T>(),
    );
    assert!(trivial);
    assert!(!vertex.inductive);
    assert!(!info.instantiable());
}

#[test]
fn visit_deep_bool() {
    let t = true;
    let f = false;
    assert_eq!(t.visit_deep().collect::<Vec<bool>>(), vec![true]);
    assert_eq!(f.visit_deep().collect::<Vec<bool>>(), vec![false]);
    assert_eq!(t.visit_deep().collect::<Vec<u64>>(), Vec::<u64>::new());
    assert_eq!(f.visit_deep().collect::<Vec<u64>>(), Vec::<u64>::new());
}

#[test]
fn visit_deep_box_bool() {
    let t = Box::new(true);
    let f = Box::new(false);
    assert_eq!(
        t.visit_deep().collect::<Vec<Box<bool>>>(),
        vec![Box::new(true)],
    );
    assert_eq!(
        f.visit_deep().collect::<Vec<Box<bool>>>(),
        vec![Box::new(false)],
    );
    assert_eq!(t.visit_deep().collect::<Vec<bool>>(), vec![true]);
    assert_eq!(f.visit_deep().collect::<Vec<bool>>(), vec![false]);
    assert_eq!(t.visit_deep().collect::<Vec<u64>>(), Vec::<u64>::new());
    assert_eq!(f.visit_deep().collect::<Vec<u64>>(), Vec::<u64>::new());
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
    let () = terms.push(Some(0x_1337_u64));
    let () = terms.push(Some(0x_1337_1337_u64));
    let () = terms.push(false);
    assert_eq!(terms.pop(), Some(false));
    assert_eq!(terms.pop(), Option::<Box<bool>>::None);
    assert_eq!(terms.pop(), Some(Some(0x_1337_1337_u64)));
    assert_eq!(terms.pop(), Some(42_u64));
    assert_eq!(terms.pop(), Option::<u64>::None);
    assert_eq!(terms.pop(), Some(true));
    assert_eq!(terms.pop(), Option::<bool>::None);
    // leave `Some(0x1337)` intact to test that `Drop` doesn't leak:
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
            .take(32)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<u64>>(),
        vec![
            u64::MAX,
            2_831_526_533_816_853,
            2780,
            198_093_122,
            3,
            40,
            10,
            1,
            2160,
            u64::MAX,
            0,
            0,
            1,
            460_252_415_470_178_231,
            0,
            65,
            331,
            u64::MAX,
            2,
            1,
            u64::MAX,
            409_780,
            11,
            0,
            0,
            30,
            74,
            7,
            3,
            0,
            5,
            11,
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
fn arbitrary_option_u64() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<Option<u64>>>(),
        vec![
            None,
            None,
            None,
            None,
            Some(23),
            Some(0),
            Some(161),
            None,
            Some(0),
            None,
        ],
    );
}

#[test]
fn arbitrary_vec_bool() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<Vec<bool>>>(),
        vec![
            vec![],
            vec![],
            vec![true],
            vec![],
            vec![false],
            vec![false, true, true, true],
            vec![true, false, false],
            vec![true, true, true, true, false],
            vec![false, false, false],
            vec![false, true, true, true],
        ],
    );
}

#[test]
fn arbitrary_btree_set_u64() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<BTreeSet<u64>>>(),
        vec![
            BTreeSet::new(),
            BTreeSet::new(),
            [1].into_iter().collect(),
            BTreeSet::new(),
            BTreeSet::new(),
            [414].into_iter().collect(),
            [0, 10_410_362_529].into_iter().collect(),
            [0, 849_508_256_479_470_101].into_iter().collect(),
            [0, 1, 5, 22].into_iter().collect(),
            [1].into_iter().collect(),
        ],
    );
}

#[test]
fn arbitrary_btree_map_u64_u64() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<BTreeMap<u64, u64>>>(),
        vec![
            BTreeMap::new(),
            BTreeMap::new(),
            [(4, 1)].into_iter().collect(),
            [(40, 1), (358, 79)].into_iter().collect(),
            [(3, 1), (11, 28_884), (46, 75)].into_iter().collect(),
            BTreeMap::new(),
            [(1_218_752_142, 55)].into_iter().collect(),
            [(0, 51_258_761), (2, 0)].into_iter().collect(),
            [(0, 0), (2_160, 0), (24_697, 5)].into_iter().collect(),
            BTreeMap::new(),
        ],
    );
}

#[test]
fn arbitrary_hash_set_u64() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<HashSet<u64>>>(),
        vec![
            HashSet::new(),
            HashSet::new(),
            [1].into_iter().collect(),
            HashSet::new(),
            HashSet::new(),
            [414].into_iter().collect(),
            [0, 10_410_362_529].into_iter().collect(),
            [0, 849_508_256_479_470_101].into_iter().collect(),
            [0, 1, 5, 22].into_iter().collect(),
            [1].into_iter().collect(),
        ],
    );
}

#[test]
fn arbitrary_hash_map_u64_u64() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<HashMap<u64, u64>>>(),
        vec![
            HashMap::new(),
            HashMap::new(),
            [(4, 1)].into_iter().collect(),
            [(40, 1), (358, 79)].into_iter().collect(),
            [(3, 1), (11, 28_884), (46, 75)].into_iter().collect(),
            HashMap::new(),
            [(1_218_752_142, 55)].into_iter().collect(),
            [(0, 51_258_761), (2, 0)].into_iter().collect(),
            [(0, 0), (2_160, 0), (24_697, 5)].into_iter().collect(),
            HashMap::new(),
        ],
    );
}

#[test]
fn arbitrary_infallible() {
    let mut prng = WyRand::new(u64::from(SEED));
    assert_eq!(
        Size::expanding()
            .take(10)
            .filter_map(|size| arbitrary(&mut prng, size))
            .collect::<Vec<Infallible>>(),
        Vec::<Infallible>::new(),
    );
}

#[test]
fn eta_expansion_bool() {
    let () = check_eta_expansion::<bool>();
}

#[test]
fn eta_expansion_box_bool() {
    let () = check_eta_expansion::<Box<bool>>();
}

#[test]
fn eta_expansion_option_u64() {
    let () = check_eta_expansion::<Option<u64>>();
}

#[test]
fn eta_expansion_vec_u64() {
    let () = check_eta_expansion::<Vec<u64>>();
}

#[test]
fn eta_expansion_btree_set_u64() {
    let () = check_eta_expansion::<BTreeSet<u64>>();
}

#[test]
fn eta_expansion_btree_map_u64() {
    let () = check_eta_expansion::<BTreeMap<u64, u64>>();
}

#[test]
fn eta_expansion_hash_set_u64() {
    let () = check_eta_expansion::<BTreeSet<u64>>();
}

#[test]
fn eta_expansion_hash_map_u64() {
    let () = check_eta_expansion::<BTreeMap<u64, u64>>();
}

#[test]
fn eta_expansion_infallible() {
    let () = check_eta_expansion::<Infallible>();
}

#[test]
fn breadth_first_iteration() {
    let values: Vec<u64> = vec![10, 20, 40];
    let iterators = values.into_iter().map(|u| (u, shrink(u))).collect();
    let iterator = breadth_first_transpose(iterators);
    let iterated: Vec<Vec<u64>> = iterator.collect();
    assert_eq!(
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

#[test]
fn shrink_option_u64() {
    assert_eq!(shrink(None).collect::<Vec<Option<u64>>>(), vec![]);
    assert_eq!(shrink(Some(0)).collect::<Vec<Option<u64>>>(), vec![None]);
    assert_eq!(
        shrink(Some(100)).collect::<Vec<Option<u64>>>(),
        vec![
            None,
            Some(0),
            Some(50),
            Some(75),
            Some(88),
            Some(94),
            Some(97),
            Some(99),
        ],
    );
}

#[test]
fn shrink_vec_u64() {
    assert_eq!(
        shrink(vec![]).collect::<Vec<Vec<u64>>>(),
        Vec::<Vec<u64>>::new(),
    );
    assert_eq!(
        shrink(vec![100]).collect::<Vec<Vec<u64>>>(),
        vec![
            vec![],
            vec![], // <-- artifact of special-casing `Vec<_>`
            vec![0],
            vec![50],
            vec![75],
            vec![88],
            vec![94],
            vec![97],
            vec![99],
        ]
    );
    assert_eq!(
        shrink(vec![10, 20, 40]).collect::<Vec<Vec<u64>>>(),
        vec![
            vec![10, 20],
            vec![10],
            vec![],
            vec![], // <-- artifact of special-casing `Vec<_>`
            vec![10, 20, 0],
            vec![10, 40],
            vec![10, 20, 20],
            vec![40],
            vec![10, 20, 30],
            vec![40],
            vec![10, 20, 35],
            vec![10, 0, 40],
            vec![10, 20, 38],
            vec![20, 40],
            vec![10, 20, 39],
            vec![10, 10, 40],
            vec![20, 40],
            vec![10, 15, 40],
            vec![0, 20, 40],
            vec![10, 18, 40],
            vec![5, 20, 40],
            vec![10, 19, 40],
            vec![8, 20, 40],
            vec![9, 20, 40],
        ],
    );
}

#[test]
fn search_witness_vec_contains_42() {
    let witness = witness(10_000, |v: &Vec<u64>| v.contains(&42)).expect("witness not found");
    assert_eq!(witness, vec![42]);
}

#[test]
fn search_witness_vec_contains_u64_max() {
    let witness = witness(10_000, |v: &Vec<u64>| v.contains(&u64::MAX)).expect("witness not found");
    assert_eq!(witness, vec![u64::MAX]);
}

#[test]
fn search_witness_infallible() {
    let maybe_witness: Option<Infallible> = witness(usize::MAX, |_| true);
    assert_eq!(maybe_witness, None);
}
