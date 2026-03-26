#![expect(clippy::panic, reason = "failing tests ought to panic")]

use {
    crate::{
        construct::Construct as _,
        hash::empty_set,
        reflection::{Constructors, TypeInfo, type_of},
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
