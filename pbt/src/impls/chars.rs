//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variants},
        registration::Registration,
    },
    core::iter,
    wyrand::WyRand,
};

/// Number of Unicode scalar values representable by `char`.
const N_UNICODE_SCALARS: u64 = 0x10_F800;

/// Largest multiple of `N_UNICODE_SCALARS` below `u64::MAX`.
const UNBIASED_LIMIT: u64 = unbiased_limit();

impl Pbt for char {
    #[inline]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
    where
        F: Fields,
    {
        debug_assert_eq!(variant_index, None, "`char` is a literal");
        fields.field()
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: None,
        }
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
        Variants::Literal {
            deserialize: |json| {
                let serde_json::Value::String(ref s) = *json else {
                    return None;
                };
                s.parse().ok()
            },
            generators: vec![uniform, ascii],
            serialize: |&i| i.to_string().into(),
            shrink,
        }
    }
}

/// Generate ASCII characters uniformly.
#[inline]
fn ascii(prng: &mut WyRand) -> char {
    #[expect(
        clippy::as_conversions,
        reason = "masking to seven bits makes the conversion lossless"
    )]
    let byte = (prng.rand() & 0x7F) as u8;
    char::from(byte)
}

/// Map an index bijectively onto the Unicode scalar values.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "The branch bounds the addition by char::MAX."
)]
fn scalar_from_index(index: u32) -> char {
    // Rust defines `char` as U+0000..=D7FF followed by U+E000..=10FFFF:
    // https://doc.rust-lang.org/core/primitive.char.html#validity-and-layout
    //
    // Indices below D800 map to the first range unchanged. Adding 800 to every
    // remaining index skips the surrogate range and maps the final index,
    // 10F7FF, to 10FFFF. This is therefore a bijection over every valid `char`.
    let scalar = if index < 0xD800 { index } else { index + 0x800 };
    // SAFETY: The bijection above produces exactly the documented scalar ranges.
    unsafe { char::from_u32_unchecked(scalar) }
}

/// Shrink a `char` by repeatedly subtracting half the previous shrunk amount.
#[inline]
fn shrink(c: char) -> Box<dyn Iterator<Item = char>> {
    let n = u32::from(c);
    let mut shift = 0;
    Box::new(
        iter::from_fn(move || {
            let delta = n.checked_shr(shift)?;
            if delta == 0 {
                return None;
            }
            shift = shift.checked_add(1)?;
            n.checked_sub(delta)
        })
        .filter_map(|u32| char::try_from(u32).ok()),
    )
}

/// Compute the rejection-sampling limit without modulo bias.
#[inline]
#[cfg_attr(test, mutants::skip)] // Arithmetic mutations can make generation loop forever.
#[expect(
    clippy::integer_division_remainder_used,
    reason = "The remainder cannot exceed u64::MAX; subtracting it yields the largest exact multiple."
)]
const fn unbiased_limit() -> u64 {
    u64::MAX - u64::MAX % N_UNICODE_SCALARS
}

/// Generate Unicode scalar values uniformly.
#[inline]
fn uniform(prng: &mut WyRand) -> char {
    'rejection_sampling: loop {
        let sample = prng.rand();
        if sample >= UNBIASED_LIMIT {
            continue 'rejection_sampling;
        }
        #[expect(
            clippy::as_conversions,
            clippy::integer_division_remainder_used,
            reason = "Modulo by the scalar count yields an in-range index; the cast is then lossless."
        )]
        let index = (sample % N_UNICODE_SCALARS) as u32;
        return scalar_from_index(index);
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        super::*,
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn ascii_generator_is_ascii() {
        let mut prng = WyRand::new(42);
        for _ in 0_usize..10_000 {
            assert!(ascii(&mut prng).is_ascii());
        }
    }

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<char> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<char> = vec![
            'j',
            'N',
            '\u{e}',
            '\u{300e6}',
            '\u{5cd58}',
            '\u{613a8}',
            'F',
            'e',
            'H',
            'i',
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn deterministic_shrink() {
        let mut iter = shrink('z');
        assert_eq!(iter.next(), Some('\0'));
        assert_eq!(iter.next(), Some('='));
        assert_eq!(iter.next(), Some('\\'));
        assert_eq!(iter.next(), Some('k'));
        assert_eq!(iter.next(), Some('s'));
        assert_eq!(iter.next(), Some('w'));
        assert_eq!(iter.next(), Some('y'));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<char>();
    }

    #[test]
    fn scalar_index_mapping_boundaries() {
        assert_eq!(scalar_from_index(0), '\0');
        assert_eq!(scalar_from_index(0xD7FF), '\u{D7FF}');
        assert_eq!(scalar_from_index(0xD800), '\u{E000}');
        assert_eq!(scalar_from_index(0x10_F7FF), char::MAX);
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<char>();
    }

    #[test]
    #[expect(
        clippy::integer_division_remainder_used,
        reason = "This test verifies that the rejection-sampling limit is an exact multiple."
    )]
    fn unbiased_limit_is_the_largest_multiple() {
        assert_eq!(UNBIASED_LIMIT, 0xFFFF_FFFF_FFFF_4800);
        assert_eq!(UNBIASED_LIMIT % N_UNICODE_SCALARS, 0);
        assert!(u64::MAX.checked_sub(UNBIASED_LIMIT).unwrap() < N_UNICODE_SCALARS);
    }
}
