//! Generation throughput for an inductive lambda-calculus syntax tree.

use {
    core::hint::black_box,
    criterion::{Criterion, Throughput, criterion_group, criterion_main},
    pbt::Pbt,
};

/// The lambda calculus with de Bruijn indices.
#[derive(Clone, Debug, Pbt)]
enum LambdaCalculus {
    /// Apply one term to another.
    Application(Box<Self>, Box<Self>),
    /// Bind one variable in a body.
    Lambda {
        /// The body under the binder.
        body: Box<Self>,
    },
    /// Refer to a surrounding binder.
    Variable {
        /// The number of binders between this variable and its binder.
        de_bruijn: usize,
    },
}

/// Measure complete generation, including periodic swarm construction.
fn generate_10_000(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("lambda_calculus");
    let _: &mut _ = group.throughput(Throughput::Elements(10_000));
    let _: &mut _ = group.bench_function("generate_10_000", |bencher| {
        bencher.iter(|| {
            let mut prng = pbt::WyRand::new(42);
            let witness = pbt::witness(
                |term: &LambdaCalculus| {
                    black_box(term);
                    None::<()>
                },
                10_000,
                &mut prng,
            );
            black_box(witness);
        });
    });
    let () = group.finish();
}

criterion_group!(benches, generate_10_000);
criterion_main!(benches);
