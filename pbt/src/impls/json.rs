//! Implementations for `serde_json` types.

use {
    crate::{
        Pbt,
        coin_flips::CoinFlips,
        fields::{Fields, Store},
        impls::integers::big_uint,
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, iter, num::NonZero},
    num_bigint::BigUint,
    serde_json::{Map, Number, Value},
    wyrand::WyRand,
};

impl Pbt for Number {
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
        debug_assert_eq!(variant_index, None, "`serde_json::Number` is a literal");
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
                let Value::Number(ref number) = *json else {
                    return None;
                };
                Some(number.clone())
            },
            generators: vec![json_number],
            serialize: |number| Value::Number(number.clone()),
            shrink: shrink_number,
        }
    }
}

impl Pbt for Map<String, Value> {
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "end-users shouldn't be calling this"
    )]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
    where
        F: Fields,
    {
        let algebraic_index: usize = variant_index
            .expect("`serde_json::Map` is not a literal")
            .get();
        match algebraic_index {
            1 => Self::new(),
            2 => {
                let mut acc: Self = fields.field();
                let _old: Option<Value> = acc.insert(fields.field(), fields.field());
                acc
            }
            _ => panic!("can't instantiate variant #{algebraic_index} of `serde_json::Map`"),
        }
    }

    #[inline]
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn deconstruct(mut self) -> Parts<Store> {
        let Some(key) = self.keys().next().cloned() else {
            return Parts {
                fields: Store::new(),
                variant_index: Some(const { NonZero::new(1).unwrap() }),
            };
        };
        let Some(value) = self.remove(&key) else {
            panic!("INTERNAL ERROR (`pbt`): TOCTOU");
        };
        let mut fields = Store::new();
        let () = fields.push(value);
        let () = fields.push(key);
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: Some(const { NonZero::new(2).unwrap() }),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<String>();
        let () = registration.register::<Value>();
        Variants::Algebraic(vec![
            Variant {
                field_types: Multiset::new(),
            },
            Variant {
                field_types: [
                    TypeId::of::<Self>(),
                    TypeId::of::<String>(),
                    TypeId::of::<Value>(),
                ]
                .into_iter()
                .collect(),
            },
        ])
    }
}

impl Pbt for Value {
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "end-users shouldn't be calling this"
    )]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
    where
        F: Fields,
    {
        let algebraic_index: usize = variant_index
            .expect("`serde_json::Value` is not a literal")
            .get();
        match algebraic_index {
            1 => Self::Null,
            2 => Self::Bool(fields.field()),
            3 => Self::Number(fields.field()),
            4 => Self::String(fields.field()),
            5 => Self::Array(fields.field()),
            6 => Self::Object(fields.field()),
            _ => panic!("can't instantiate variant #{algebraic_index} of `serde_json::Value`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let variant_index = match self {
            Self::Null => const { NonZero::new(1).unwrap() },
            Self::Bool(bool) => {
                let () = fields.push(bool);
                const { NonZero::new(2).unwrap() }
            }
            Self::Number(number) => {
                let () = fields.push(number);
                const { NonZero::new(3).unwrap() }
            }
            Self::String(string) => {
                let () = fields.push(string);
                const { NonZero::new(4).unwrap() }
            }
            Self::Array(array) => {
                let () = fields.push(array);
                const { NonZero::new(5).unwrap() }
            }
            Self::Object(object) => {
                let () = fields.push(object);
                const { NonZero::new(6).unwrap() }
            }
        };
        Parts {
            fields,
            variant_index: Some(variant_index),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<bool>();
        let () = registration.register::<Number>();
        let () = registration.register::<String>();
        let () = registration.register::<Vec<Self>>();
        let () = registration.register::<Map<String, Self>>();
        Variants::Algebraic(vec![
            Variant {
                field_types: Multiset::new(),
            },
            Variant {
                field_types: iter::once(TypeId::of::<bool>()).collect(),
            },
            Variant {
                field_types: iter::once(TypeId::of::<Number>()).collect(),
            },
            Variant {
                field_types: iter::once(TypeId::of::<String>()).collect(),
            },
            Variant {
                field_types: iter::once(TypeId::of::<Vec<Self>>()).collect(),
            },
            Variant {
                field_types: iter::once(TypeId::of::<Map<String, Self>>()).collect(),
            },
        ])
    }
}

/// Generate a JSON number from a `BigUint`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "a decimal `BigUint` string is always a valid JSON number"
)]
fn number_from_biguint(biguint: &BigUint) -> Number {
    biguint
        .to_string()
        .parse()
        .expect("INTERNAL ERROR (`pbt`): invalid `BigUint` JSON number")
}

/// Generate an arbitrary `usize` with a geometric distribution.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "the hardware will die before this overflows"
)]
fn geometric_usize(coin: &mut CoinFlips, prng: &mut WyRand) -> usize {
    let mut acc = 0_usize;
    while coin.flip(prng) {
        acc += 1;
    }
    acc
}

/// Generate a `serde_json::Number` by following exactly the RFC 8259 grammar.
///
/// RFC 8259 §6 defines JSON numbers as optional minus, then an integer
/// (`0` or a nonzero digit followed by digits), then optional fraction, then
/// optional exponent. Equivalently:
/// `-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`.
///
/// Source: <https://datatracker.ietf.org/doc/html/rfc8259#section-6>.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "we generate exactly the grammar accepted by `Number::from_str`"
)]
fn json_number(prng: &mut WyRand) -> Number {
    let mut coin = CoinFlips::new(prng);
    let mut acc = String::new();
    if coin.flip(prng) {
        acc.push('-');
    }
    push_integer(&mut coin, prng, &mut acc);
    if coin.flip(prng) {
        acc.push('.');
        push_digits(&mut coin, prng, &mut acc);
    }
    if coin.flip(prng) {
        acc.push(if coin.flip(prng) { 'e' } else { 'E' });
        if coin.flip(prng) {
            acc.push(if coin.flip(prng) { '+' } else { '-' });
        }
        push_digits(&mut coin, prng, &mut acc);
    }
    acc.parse()
        .expect("INTERNAL ERROR (`pbt`): generated invalid JSON number")
}

/// Push a nonempty sequence of decimal digits.
#[inline]
fn push_digits(coin: &mut CoinFlips, prng: &mut WyRand, out: &mut String) {
    let n_zeros = geometric_usize(coin, prng);
    let () = out.extend(iter::repeat_n('0', n_zeros));
    let () = out.push_str(&big_uint(coin, prng, 1).to_string());
}

/// Push an RFC 8259 integer component.
#[inline]
#[expect(clippy::arithmetic_side_effects, reason = "`BigUint` cannot overflow")]
fn push_integer(coin: &mut CoinFlips, prng: &mut WyRand, out: &mut String) {
    if coin.flip(prng) {
        out.push('0');
    } else {
        out.push_str(&(big_uint(coin, prng, 1) + BigUint::ONE).to_string());
    }
}

/// Shrink unsigned-integer JSON numbers using `BigUint` arithmetic.
#[inline]
#[expect(
    clippy::needless_pass_by_value,
    reason = "literal shrinking functions take owned values"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "`BigUint` cannot underflow here"
)]
fn shrink_number(number: Number) -> Box<dyn Iterator<Item = Number>> {
    let Ok(n) = number.to_string().parse::<BigUint>() else {
        return Box::new(iter::empty());
    };
    let mut shift = 0_usize;
    Box::new(iter::from_fn(move || {
        let delta = &n >> shift;
        if delta == BigUint::from(0_u8) {
            return None;
        }
        shift += 1;
        let shrunk = &n - delta;
        Some(number_from_biguint(&shrunk))
    }))
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        super::*,
        crate::{check_eta_expansion, check_serialization},
    };

    #[test]
    fn number_eta_expansion() {
        let () = check_eta_expansion::<Number>();
    }

    #[test]
    fn number_serialization() {
        let () = check_serialization::<Number>();
    }

    #[test]
    fn number_from_biguint_exceeds_u64() {
        let biguint: BigUint = "18446744073709551616".parse().unwrap();
        let number = number_from_biguint(&biguint);
        assert!(biguint > BigUint::from(u64::MAX));
        assert_eq!(number.to_string(), biguint.to_string());
    }

    #[test]
    fn deterministic_big_uint() {
        let mut prng = WyRand::new(123);
        let mut coin = CoinFlips::new(&mut prng);
        let generated: Vec<BigUint> = iter::repeat_with(|| big_uint(&mut coin, &mut prng, 1))
            .take(10_usize)
            .collect();
        let expected: Vec<BigUint> = [1_usize, 31, 0, 1, 5, 1, 1, 1, 2, 0]
            .into_iter()
            .map(BigUint::from)
            .collect();
        assert_eq!(generated, expected);
    }

    #[test]
    fn deterministic_geometric_usize() {
        let mut prng = WyRand::new(123);
        let mut coin = CoinFlips::new(&mut prng);
        let generated: Vec<usize> = iter::repeat_with(|| geometric_usize(&mut coin, &mut prng))
            .take(10_usize)
            .collect();
        assert_eq!(generated, vec![1, 9, 0, 1, 2, 2, 1, 1, 1, 2]);
    }

    #[test]
    fn deterministic_json_numbers() {
        let mut prng = WyRand::new(123);
        let generated: Vec<String> = iter::repeat_with(|| json_number(&mut prng).to_string())
            .take(5_usize)
            .collect();
        assert_eq!(generated, vec!["-32e+33", "0e+07", "-2", "1", "-2.000e+1"],);
    }

    #[test]
    fn deterministic_push_digits() {
        let mut prng = WyRand::new(123);
        let mut coin = CoinFlips::new(&mut prng);
        let generated: Vec<String> = iter::repeat_with(|| {
            let mut s = String::new();
            let () = push_digits(&mut coin, &mut prng, &mut s);
            s
        })
        .take(10_usize)
        .collect();
        assert_eq!(
            generated,
            vec!["031", "1", "0033", "0", "0", "02", "001", "012", "00", "1",],
        );
    }

    #[test]
    fn deterministic_push_integer() {
        let mut prng = WyRand::new(123);
        let mut coin = CoinFlips::new(&mut prng);
        let generated: Vec<String> = iter::repeat_with(|| {
            let mut s = String::new();
            let () = push_integer(&mut coin, &mut prng, &mut s);
            s
        })
        .take(10_usize)
        .collect();
        assert_eq!(
            generated,
            vec!["0", "32", "2", "0", "0", "34", "1", "1", "0", "3"],
        );
    }

    #[test]
    fn deterministic_shrink_number() {
        let original = number_from_biguint(&BigUint::from(1_000_usize));
        let generated: Vec<String> = shrink_number(original)
            .take(11_usize)
            .map(|candidate| candidate.to_string())
            .collect();
        assert_eq!(
            generated,
            vec![
                "0", "500", "750", "875", "938", "969", "985", "993", "997", "999"
            ],
        );
    }

    #[test]
    fn map_eta_expansion() {
        let () = check_eta_expansion::<Map<String, Value>>();
    }

    #[test]
    fn map_serialization() {
        let () = check_serialization::<Map<String, Value>>();
    }

    #[test]
    fn value_eta_expansion() {
        let () = check_eta_expansion::<Value>();
    }

    #[test]
    fn value_serialization() {
        let () = check_serialization::<Value>();
    }
}
