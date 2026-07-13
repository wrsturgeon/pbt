//! Whole-program access to the live LLVM coverage counters.

/// Linux ELF access to the executable's live LLVM profile counter section.
mod live_counters {
    use {
        super::CoverageTrace,
        core::{mem, ptr, slice},
    };

    unsafe extern "C" {
        /// First `u64` in the linker-concatenated LLVM profile counter section.
        #[link_name = "__start___llvm_prf_cnts"]
        static mut COUNTERS_START: u64;

        /// One-past-the-end address of the LLVM profile counter section.
        #[link_name = "__stop___llvm_prf_cnts"]
        static mut COUNTERS_END: u64;

        /// Reset the executable's live LLVM profiling state.
        #[link_name = "__llvm_profile_reset_counters"]
        fn reset_counters();
    }

    /// Run a closure with a scoped immutable view of every physical counter.
    ///
    /// # Safety
    ///
    /// No instrumented code may execute, reset the counters, or access them
    /// from another thread until `read` returns.
    #[coverage(off)]
    #[expect(
        clippy::panic,
        reason = "invalid linker-provided counter bounds must fail loudly"
    )]
    unsafe fn with_counters<R>(read: impl for<'counter> FnOnce(&'counter [u64]) -> R) -> R {
        let start = ptr::addr_of!(COUNTERS_START);
        let end = ptr::addr_of!(COUNTERS_END);

        // The GNU-style ELF linker synthesizes these symbols at the boundaries
        // of one output section. Their ordered addresses therefore delimit one
        // contiguous byte range, so checked integer subtraction gives its size.
        let Some(byte_length) = end.addr().checked_sub(start.addr()) else {
            panic!("LLVM profile counter section ends before it starts");
        };
        let counter_width = mem::size_of::<u64>();
        assert!(
            start.addr().is_multiple_of(mem::align_of::<u64>()),
            "LLVM profile counter section is not aligned for `u64`",
        );
        assert!(
            byte_length.is_multiple_of(counter_width),
            "LLVM profile counter section is not an array of `u64`",
        );
        assert!(
            byte_length <= isize::MAX.unsigned_abs(),
            "LLVM profile counter section is too large for a Rust slice",
        );

        let Some(counter_count) = byte_length.checked_div(counter_width) else {
            panic!("LLVM profile counter width must be nonzero");
        };
        assert!(
            counter_count > 0,
            "LLVM profile counter section is empty; was instrumentation enabled?",
        );
        assert!(
            u32::try_from(counter_count).is_ok(),
            "LLVM physical counter count does not fit in `u32`",
        );

        // SAFETY:
        // - `-C instrument-coverage` emits each physical counter as an aligned
        //   `u64` in `__llvm_prf_cnts`, and the ELF linker concatenates those
        //   input sections between the two boundary symbols above.
        // - The checked address subtraction and divisibility assertions prove
        //   that `counter_count` spans exactly that contiguous output section.
        // - The caller guarantees that no LLVM counter mutation is possible
        //   while the immutable slice exists.
        // - The higher-ranked callback keeps the slice borrow scoped to this
        //   call, so it cannot remain alive across a later reset or execution.
        let counters = unsafe { slice::from_raw_parts(start, counter_count) };
        read(counters)
    }

    /// Return the executable's number of physical LLVM counters.
    #[coverage(off)]
    pub(super) fn physical_counter_count() -> usize {
        // SAFETY: The spike selects exactly one test and one test thread. This
        // coverage-disabled callback only reads the slice length and returns
        // before any observed execution or reset begins.
        unsafe { with_counters(<[u64]>::len) }
    }

    /// Reset all live LLVM profiling counters.
    #[coverage(off)]
    pub(super) fn reset() {
        // SAFETY: No counter slice is alive when this function is called. The
        // single-threaded observation sequence is always reset, execute, then
        // snapshot, so LLVM has exclusive access while resetting its state.
        unsafe {
            reset_counters();
        }
    }

    /// Copy the sorted indices of every currently nonzero physical counter.
    #[coverage(off)]
    #[expect(
        clippy::panic,
        reason = "a counter index that cannot be represented must fail loudly"
    )]
    pub(super) fn snapshot() -> CoverageTrace {
        let mut locations = Vec::new();
        // SAFETY: The spike selects exactly one test and one test thread. Both
        // this function and its callback have coverage disabled, and the
        // callback only reads counters before returning the scoped borrow.
        unsafe {
            with_counters(|counters| {
                for (physical_index, &count) in counters.iter().enumerate() {
                    if count == 0 {
                        continue;
                    }
                    let Ok(location) = u32::try_from(physical_index) else {
                        panic!("LLVM physical counter index does not fit in `u32`");
                    };
                    locations.push(location);
                }
            });
        }
        CoverageTrace {
            locations: locations.into_boxed_slice(),
        }
    }
}

use {
    core::{hint::black_box, panic::AssertUnwindSafe, time::Duration},
    pbt::multiset::Multiset,
    std::{panic, time::Instant},
};

#[cfg(not(target_os = "linux"))]
compile_error!("the whole-program coverage instrumentation spike supports Linux only");

/// How many times each isolated timing measurement is repeated.
const MEASUREMENT_ITERATIONS: u32 = 2_000;

/// The set of physical LLVM counters observed during one execution.
#[derive(Debug, Eq, PartialEq)]
struct CoverageTrace {
    /// Sorted physical counter indices, each appearing exactly once.
    locations: Box<[u32]>,
}

/// The result and trace of one observed execution.
struct Observation {
    /// Whether the observed body unwound.
    panicked: bool,
    /// The counters touched before the body returned or finished unwinding.
    trace: CoverageTrace,
}

/// Fixed inputs selecting deliberately different control-flow paths.
#[derive(Clone, Copy)]
enum FixedPath {
    /// Insert three distinct values.
    Distinct,
    /// Insert the same value twice.
    Duplicate,
    /// Enter a recognizable path and then panic.
    Panic,
}

/// Execute deterministic source-built dependency paths.
#[inline(never)]
#[expect(
    clippy::panic,
    reason = "the panic variant exists to verify coverage capture during unwinding"
)]
fn observed_body(path: FixedPath) {
    let mut values = Multiset::new();
    values.insert(7_u8);

    match path {
        FixedPath::Duplicate => values.insert(7),
        FixedPath::Distinct => {
            values.insert(11);
            values.insert(13);
        }
        FixedPath::Panic => {
            values.insert(17);
            black_box(&values);
            panic!("recognizable whole-program coverage spike path");
        }
    }

    black_box(values);
}

/// Reset, execute one fixed input, and snapshot after return or unwind.
#[coverage(off)]
fn observe(path: FixedPath) -> Observation {
    live_counters::reset();
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| observed_body(path)));
    let trace = live_counters::snapshot();
    Observation {
        panicked: outcome.is_err(),
        trace,
    }
}

/// Return whether a trace satisfies its representation invariants.
#[coverage(off)]
fn trace_is_valid(trace: &CoverageTrace, physical_counter_count: u32) -> bool {
    trace
        .locations
        .iter()
        .all(|&index| index < physical_counter_count)
        && trace
            .locations
            .windows(2)
            .all(|pair| matches!(pair, [lower, upper] if lower < upper))
}

/// Return how many indices occur in exactly one of two sorted traces.
#[coverage(off)]
#[expect(
    clippy::panic,
    reason = "an impossible symmetric-difference overflow must fail loudly"
)]
fn symmetric_difference_count(lhs: &CoverageTrace, rhs: &CoverageTrace) -> usize {
    let lhs_only = lhs
        .locations
        .iter()
        .filter(|index| rhs.locations.binary_search(index).is_err())
        .count();
    let rhs_only = rhs
        .locations
        .iter()
        .filter(|index| lhs.locations.binary_search(index).is_err())
        .count();
    let Some(total) = lhs_only.checked_add(rhs_only) else {
        panic!("trace symmetric difference overflowed `usize`");
    };
    total
}

/// Measure repeated resets independently of body execution and scanning.
#[coverage(off)]
fn measure_resets() -> Duration {
    live_counters::reset();
    let started = Instant::now();
    for _ in 0..MEASUREMENT_ITERATIONS {
        live_counters::reset();
    }
    started.elapsed()
}

/// Measure repeated executions independently of resets and scanning.
#[coverage(off)]
fn measure_bodies() -> Duration {
    live_counters::reset();
    let started = Instant::now();
    for _ in 0..MEASUREMENT_ITERATIONS {
        observed_body(FixedPath::Duplicate);
    }
    let elapsed = started.elapsed();
    live_counters::reset();
    elapsed
}

/// Measure repeated scans over one representative sparse trace.
#[coverage(off)]
fn measure_snapshots() -> Duration {
    live_counters::reset();
    observed_body(FixedPath::Duplicate);
    let started = Instant::now();
    for _ in 0..MEASUREMENT_ITERATIONS {
        black_box(live_counters::snapshot());
    }
    let elapsed = started.elapsed();
    live_counters::reset();
    elapsed
}

/// Measure complete reset, execute, and snapshot observations.
#[coverage(off)]
fn measure_observations() -> Duration {
    let started = Instant::now();
    for _ in 0..MEASUREMENT_ITERATIONS {
        black_box(observe(FixedPath::Duplicate));
    }
    started.elapsed()
}

/// Average a repeated measurement over the fixed iteration count.
#[coverage(off)]
#[expect(
    clippy::panic,
    reason = "the fixed measurement iteration count must remain nonzero"
)]
fn average_duration(total: Duration) -> Duration {
    let Some(average) = total.checked_div(MEASUREMENT_ITERATIONS) else {
        panic!("measurement iteration count must be nonzero");
    };
    average
}

/// Verify deterministic live counter access and report its costs.
#[test]
#[coverage(off)]
#[expect(
    clippy::panic,
    reason = "an unrepresentable physical counter count must fail loudly"
)]
#[expect(
    clippy::print_stdout,
    clippy::use_debug,
    reason = "the spike must print its measured durations under `--nocapture`"
)]
fn whole_program_coverage_spike() {
    let physical_counter_count = live_counters::physical_counter_count();
    let Ok(physical_counter_count_u32) = u32::try_from(physical_counter_count) else {
        panic!("LLVM physical counter count does not fit in `u32`");
    };

    live_counters::reset();
    let baseline = live_counters::snapshot();
    assert!(
        baseline.locations.is_empty(),
        "coverage-disabled observation machinery produced a nonzero baseline",
    );

    let duplicate_first = observe(FixedPath::Duplicate);
    let duplicate_second = observe(FixedPath::Duplicate);
    assert!(!duplicate_first.panicked);
    assert!(!duplicate_second.panicked);
    assert_eq!(duplicate_first.trace, duplicate_second.trace);
    assert_eq!(
        symmetric_difference_count(&duplicate_first.trace, &duplicate_second.trace),
        0,
        "identical traces produced a nonempty symmetric difference",
    );

    let distinct = observe(FixedPath::Distinct);
    assert!(!distinct.panicked);
    assert_ne!(duplicate_first.trace, distinct.trace);
    let branch_symmetric_difference =
        symmetric_difference_count(&duplicate_first.trace, &distinct.trace);
    assert!(
        branch_symmetric_difference > 0,
        "intentionally different branches produced no symmetric difference",
    );

    let panic = observe(FixedPath::Panic);
    assert!(panic.panicked);
    assert!(
        !panic.trace.locations.is_empty(),
        "panic path produced an empty trace",
    );

    let post_panic = observe(FixedPath::Duplicate);
    assert!(!post_panic.panicked);
    assert_eq!(
        post_panic.trace, duplicate_first.trace,
        "panic coverage leaked through the following reset",
    );

    for trace in [
        &baseline,
        &duplicate_first.trace,
        &duplicate_second.trace,
        &distinct.trace,
        &panic.trace,
        &post_panic.trace,
    ] {
        assert!(
            trace_is_valid(trace, physical_counter_count_u32),
            "observed trace violates its representation invariants",
        );
    }
    assert!(
        !trace_is_valid(
            &CoverageTrace {
                locations: Box::new([physical_counter_count_u32]),
            },
            physical_counter_count_u32,
        ),
        "trace validation accepted an out-of-range counter index",
    );
    assert!(
        !trace_is_valid(
            &CoverageTrace {
                locations: Box::new([0, 0]),
            },
            physical_counter_count_u32,
        ),
        "trace validation accepted duplicate counter indices",
    );

    let reset_duration = measure_resets();
    let body_duration = measure_bodies();
    let snapshot_duration = measure_snapshots();
    let observation_duration = measure_observations();
    let reset_average = average_duration(reset_duration);
    let body_average = average_duration(body_duration);
    let snapshot_average = average_duration(snapshot_duration);
    let observation_average = average_duration(observation_duration);

    for (measurement, total, average) in [
        ("reset", reset_duration, reset_average),
        ("body", body_duration, body_average),
        ("snapshot/scan", snapshot_duration, snapshot_average),
        (
            "aggregate observation",
            observation_duration,
            observation_average,
        ),
    ] {
        assert!(
            !total.is_zero(),
            "{measurement} measurement produced a zero total duration",
        );
        assert!(
            !average.is_zero(),
            "{measurement} measurement produced a zero average duration",
        );
    }

    println!(
        "\
whole-program coverage instrumentation spike
  physical counters: {physical_counter_count}
  observation baseline: {}
  duplicate-path nonzero counters: {}
  repeated duplicate-path nonzero counters: {}
  distinct-path nonzero counters: {}
  panic-path nonzero counters: {}
  branch symmetric difference: {branch_symmetric_difference}
  measurement iterations: {MEASUREMENT_ITERATIONS}
  reset duration: {reset_duration:?} total ({:?} per reset)
  body duration: {body_duration:?} total ({:?} per execution)
  snapshot/scan duration: {snapshot_duration:?} total ({:?} per scan)
  aggregate throughput: {MEASUREMENT_ITERATIONS} observations in {observation_duration:?} ({:?} per observation)",
        baseline.locations.len(),
        duplicate_first.trace.locations.len(),
        duplicate_second.trace.locations.len(),
        distinct.trace.locations.len(),
        panic.trace.locations.len(),
        reset_average,
        body_average,
        snapshot_average,
        observation_average,
    );
}
