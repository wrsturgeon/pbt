#[inline]
pub fn bernoulli<Rng: rand_core::RngCore>(rng: &mut Rng, pr_true: f32) -> bool {
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        reason = "intentional"
    )]
    let threshold = (pr_true * const { u32::MAX as f32 }) as u32;
    rng.next_u32() < threshold
}

#[cfg(test)]
mod test {
    use {super::*, crate::traits::rnd::default_rng};

    #[test]
    fn bernoulli_accurate() {
        const N_TRIALS: usize = 10_000;

        for pr_true in [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0] {
            let mean = {
                let mut rng = default_rng();
                let mut acc = 0;
                for _ in 0..N_TRIALS {
                    acc += usize::from(bernoulli(&mut rng, pr_true));
                }
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_precision_loss,
                    reason = "intentional"
                )]
                {
                    acc as f32 / const { N_TRIALS as f32 }
                }
            };
            let error = mean - pr_true;
            assert!(
                error.abs() < 0.01,
                "Expected `Pr[true] = {pr_true:.1}` but found {mean:.3} ({:.0}% error)!",
                error * 100.,
            );
        }
    }
}
