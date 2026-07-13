//! Registers the private conditional configuration used by the coverage spike.

use std::env;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pbt_coverage_spike)");
    println!("cargo::rerun-if-env-changed=PBT_COVERAGE_SPIKE");

    let Some(value) = env::var_os("PBT_COVERAGE_SPIKE") else {
        return;
    };
    assert_eq!(
        value, "1",
        "PBT_COVERAGE_SPIKE must be unset or exactly `1`",
    );

    let instrumented = env::var("CARGO_ENCODED_RUSTFLAGS").is_ok_and(|flags| {
        flags
            .split('\u{1f}')
            .any(|flag| flag == "-Cinstrument-coverage")
    });
    assert!(
        instrumented,
        "PBT_COVERAGE_SPIKE=1 requires `-Cinstrument-coverage` in RUSTFLAGS",
    );

    println!("cargo::rustc-cfg=pbt_coverage_spike");
}
