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
    std::{
        collections::{BTreeMap, BTreeSet, HashMap, HashSet},
        rc::Rc,
        sync::Arc,
    },
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
        ..
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Literal { .. } = *type_former else {
        panic!("expected literal (non-algebraic) type former but found {type_former:#?}")
    };
    assert_eq!(
        vertex.ty,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.ty.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert!(trivial);
    assert!(!vertex.is_inductive());
    assert!(!info.is_big());
    assert!(info.instantiable(&mut BTreeSet::new()));
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
        ..
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
        vertex.ty,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.ty.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(vertex.unavoidable, iter::once(type_of::<bool>()).collect(),);
    assert!(trivial);
    assert!(!vertex.is_inductive());
    assert!(!info.is_big());
    assert!(info.instantiable(&mut BTreeSet::new()));
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
        ..
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
        vertex.ty,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.ty.id(),
        TypeId::of::<T>(),
    );
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert!(!trivial);
    assert!(!vertex.is_inductive());
    assert!(!info.is_big());
    assert!(info.instantiable(&mut BTreeSet::new()));
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
        ..
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert_eq!(constructors.all_constructors.len(), 2);
    assert!(!constructors.all_constructors[0].1.is_inductive());
    assert_eq!(
        constructors.all_constructors[0].1.vertex.unavoidable,
        BTreeSet::new(),
    );
    assert!(constructors.all_constructors[1].1.is_inductive());
    assert_eq!(
        constructors.all_constructors[1].1.vertex.unavoidable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(constructors.guaranteed_leaves().len(), 1);
    assert_eq!(constructors.guaranteed_loops().len(), 1);
    assert_eq!(constructors.potential_leaves().len(), 1);
    assert_eq!(constructors.potential_loops().len(), 1);
    assert_eq!(
        vertex.ty,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.ty.id(),
        TypeId::of::<T>(),
    );
    assert!(!trivial);
    assert!(vertex.is_inductive());
    assert!(info.is_big());
    assert!(info.instantiable(&mut BTreeSet::new()));
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
        ..
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
    assert_eq!(vertex.unavoidable, BTreeSet::new());
    assert_eq!(constructors.all_constructors.len(), 2);
    assert!(!constructors.all_constructors[0].1.is_inductive());
    assert_eq!(
        constructors.all_constructors[0].1.vertex.unavoidable,
        BTreeSet::new(),
    );
    assert!(constructors.all_constructors[1].1.is_inductive());
    assert_eq!(
        constructors.all_constructors[1].1.vertex.unavoidable,
        [type_of::<u64>(), type_of::<T>()].into_iter().collect(),
    );
    assert_eq!(constructors.guaranteed_leaves().len(), 1);
    assert_eq!(constructors.guaranteed_loops().len(), 1);
    assert_eq!(constructors.potential_leaves().len(), 1);
    assert_eq!(constructors.potential_loops().len(), 1);
    assert_eq!(
        vertex.ty,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.ty.id(),
        TypeId::of::<T>(),
    );
    assert!(!trivial);
    assert!(vertex.is_inductive());
    assert!(info.is_big());
    assert!(info.instantiable(&mut BTreeSet::new()));
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
        ..
    } = *info;
    assert_eq!(name, type_name::<T>());
    let PrecomputedTypeFormer::Algebraic(ref constructors) = *type_former else {
        panic!("expected algebraic constructors but found {type_former:#?}")
    };
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
        vertex.ty,
        type_of::<T>(),
        "{:?} =/= {:?}",
        vertex.ty.id(),
        TypeId::of::<T>(),
    );
    assert!(trivial);
    assert!(!vertex.is_inductive());
    assert!(!info.is_big());
    assert!(!info.instantiable(&mut BTreeSet::new()));
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
            35, 0, 1, 12, 60, 12, 1, 22787, 0, 1, 0, 5793, 1, 0, 0, 249, 0, 416, 976, 17, 0, 1, 0,
            2, 1, 2, 1, 3, 35, 4, 1, 277,
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
            Some(2),
            None,
            Some(4),
            Some(28),
            None,
            Some(0),
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
            vec![],
            vec![true],
            vec![false],
            vec![true],
            vec![false],
            vec![false, true],
            vec![true],
            vec![],
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
            BTreeSet::new(),
            [4].into_iter().collect(),
            [4].into_iter().collect(),
            [12].into_iter().collect(),
            [0].into_iter().collect(),
            [0].into_iter().collect(),
            [1, 3].into_iter().collect(),
            [0, 46].into_iter().collect(),
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
            BTreeMap::new(),
            [(1, 4)].into_iter().collect(),
            [(1, 0)].into_iter().collect(),
            [(0, 12)].into_iter().collect(),
            [(1, 0)].into_iter().collect(),
            BTreeMap::new(),
            [(2, 15), (46, 1)].into_iter().collect(),
            [(3, 15)].into_iter().collect(),
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
            HashSet::new(),
            [4].into_iter().collect(),
            [4].into_iter().collect(),
            [12].into_iter().collect(),
            [0].into_iter().collect(),
            [0].into_iter().collect(),
            [1, 3].into_iter().collect(),
            [0, 46].into_iter().collect(),
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
            HashMap::new(),
            [(1, 4)].into_iter().collect(),
            [(1, 0)].into_iter().collect(),
            [(0, 12)].into_iter().collect(),
            [(1, 0)].into_iter().collect(),
            HashMap::new(),
            [(2, 15), (46, 1)].into_iter().collect(),
            [(3, 15)].into_iter().collect(),
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
fn eta_expansion_rc_bool() {
    let () = check_eta_expansion::<Rc<bool>>();
}

#[test]
fn eta_expansion_arc_bool() {
    let () = check_eta_expansion::<Arc<bool>>();
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
fn search_witness_vec_contains_42() {
    let witness = witness(10_000, |v: &Vec<u64>| v.contains(&42)).expect("witness not found");
    assert_eq!(witness, vec![42]);
}

#[test]
fn search_witness_vec_contains_u16_max() {
    let witness = witness(10_000, |v: &Vec<u16>| v.contains(&u16::MAX)).expect("witness not found");
    assert_eq!(witness, vec![u16::MAX]);
}

#[test]
fn search_witness_infallible() {
    let maybe_witness: Option<Infallible> = witness(usize::MAX, |_| true);
    assert_eq!(maybe_witness, None);
}
