//! Generation throughput benchmarks.
//!
//! Requires Valgrind and `gungraun-runner` on `PATH`; `nix develop` provides both.
//! Run with: `cargo bench -p pbt --bench generation`.

#![expect(
    missing_docs,
    reason = "Gungraun's benchmark entry-point macros generate undocumented public items"
)]
#![expect(
    clippy::exit,
    reason = "Gungraun owns the benchmark process entry point"
)]

use {
    core::{hint::black_box, iter},
    gungraun::{
        Callgrind, EventKind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group,
        main,
    },
    pbt::{Pbt, WyRand, pbt::arbitrary, size::Size},
};

/// Base number of generated terms per benchmark case.
const N_CASES: usize = 1_000;

/// Fixed PRNG seed used for every benchmark case.
const SEED: u64 = 0x1337_5eed_f00d_cafe;

/// Lambda-calculus terms using de Bruijn indices for variables.
#[derive(Clone, Debug, Eq, PartialEq, Pbt)]
enum LambdaCalculus {
    /// Function application.
    Application(Box<Self>, Box<Self>),
    /// Lambda abstraction.
    Lambda(Box<Self>),
    /// De Bruijn variable index.
    Variable(usize),
}

/// Deterministic generation-size schedule.
#[derive(Clone, Copy)]
enum SizeSchedule {
    /// [`N_CASES`] terms at a fixed size of 1,000.
    Large,
    /// First [`N_CASES`] normal search sizes.
    Small,
}

/// Warm lazy generation state outside the measured region.
fn setup_schedule(schedule: SizeSchedule) -> SizeSchedule {
    // Run once to let `pbt` populate its global registry.
    generate_lambda_calculus_batch(iter::once(Size::new(1_000)));

    schedule
}

/// Generate one deterministic batch of lambda-calculus terms for a size schedule.
#[expect(
    clippy::panic,
    reason = "lambda-calculus terms are structurally instantiable; a failure here is a benchmark setup bug"
)]
fn generate_lambda_calculus_batch(sizes: impl Iterator<Item = Size>) {
    let mut prng = WyRand::new(SEED);
    for size in sizes {
        let Some(term) = arbitrary::<LambdaCalculus>(&mut prng, size) else {
            panic!("lambda-calculus generation unexpectedly failed");
        };
        let _term = black_box(term);
    }
}

#[library_benchmark]
#[bench::small(args = (SizeSchedule::Small), setup = setup_schedule)]
#[bench::large(args = (SizeSchedule::Large), setup = setup_schedule)]
fn lambda_calculus_generation(schedule: SizeSchedule) {
    match schedule {
        SizeSchedule::Small => {
            generate_lambda_calculus_batch(Size::expanding().take(const { 10 * N_CASES }));
        }
        SizeSchedule::Large => {
            generate_lambda_calculus_batch(iter::repeat_with(|| Size::new(1_000)).take(N_CASES));
        }
    }
}

library_benchmark_group!(
    name = generation;
    max_parallel = 1;
    benchmarks = lambda_calculus_generation
);
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default().format([EventKind::Ir, EventKind::TotalRW]));
    library_benchmark_groups = generation
);
