//! Implementation for `Vec<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            arbitrary, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        size::Size,
    },
    core::num::NonZero,
    std::collections::BTreeSet,
    wyrand::WyRand,
};

impl<Lhs: Construct, Rhs: Construct> Construct for (Lhs, Rhs) {
    #[inline]
    #[expect(clippy::expect_used, reason = "internal invariants")]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        prng: &mut WyRand,
        size: Size,
    ) -> TermsOfVariousTypes {
        let mut fields = TermsOfVariousTypes::new();
        let mut sizes = size
            .partition_into(2, prng, false)
            .expect("internal `pbt` error: partition a size into two");
        let () = fields.push(arbitrary::<Lhs>(
            prng,
            sizes
                .next()
                .expect("internal `pbt` error: partition a size into two"),
        ));
        let () = fields.push(arbitrary::<Rhs>(
            prng,
            sizes
                .next()
                .expect("internal `pbt` error: partition a size into two"),
        ));
        fields
    }

    #[inline]
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<Lhs>(visited.clone());
        let () = register::<Rhs>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            elimination_rule: ElimFn::new(|(lhs, rhs)| {
                let mut fields = TermsOfVariousTypes::new();
                let () = fields.push(lhs);
                let () = fields.push(rhs);
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
            introduction_rules: vec![IntroductionRule {
                call: CtorFn::new(|fields| (fields.must_pop(), fields.must_pop())),
                immediate_dependencies: [type_of::<Lhs>(), type_of::<Rhs>()].into_iter().collect(),
            }],
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        let (ref lhs, ref rhs) = *self;
        visit_self(self)
            .chain(lhs.visit_deep())
            .chain(rhs.visit_deep())
    }
}
