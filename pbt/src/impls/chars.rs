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
            generators: vec![uniform],
            serialize: |&i| i.to_string().into(),
            shrink,
        }
    }
}

/// Shrink an integer by repeatedly subtracting half the previous shrunk amount.
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

/// Generate integers uniformly over the target machine word.
#[inline]
fn uniform(prng: &mut WyRand) -> char {
    'rejection_sampling: loop {
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "intentional: bit width checked above"
        )]
        let u32 = prng.rand() as u32;
        let Ok(c) = char::try_from(u32) else {
            continue 'rejection_sampling;
        };
        return c;
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<char> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<char> = vec![
            '\u{fb8e8}',
            '\u{9bf28}',
            '\u{7ea5b}',
            '\u{100fee}',
            '\u{bdb4}',
            '\u{67457}',
            '\u{6db20}',
            '\u{f7975}',
            '\u{8a8c1}',
            '\u{fdc56}',
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<char>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<char>();
    }
}
