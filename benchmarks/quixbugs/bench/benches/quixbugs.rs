//! Criterion benchmarks for faithfully translated `QuixBugs` programs.

#![expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Each translation is presented in source order: buggy, corrected, then property."
)]

use {
    core::{fmt::Debug, hint::black_box},
    criterion::{
        BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime,
    },
    quixbugs_api::{Check, Library},
    quixbugs_pbt::Pbt,
    quixbugs_proptest::Proptest,
    quixbugs_quickcheck::QuickCheck,
};

#[inline]
/// Benchmark every library adapter on one translated program.
fn benchmark<T>(criterion: &mut Criterion, name: &str, property: fn(&T) -> bool)
where
    Pbt: Check<T>,
    Proptest: Check<T>,
    QuickCheck: Check<T>,
    T: Debug,
{
    let mut group = criterion.benchmark_group(name);
    benchmark_library::<Pbt, _>(&mut group, "pbt", property);
    benchmark_library::<Proptest, _>(&mut group, "proptest", property);
    benchmark_library::<QuickCheck, _>(&mut group, "quickcheck", property);
    let () = group.finish();
}

/// Benchmark one translated program through one library adapter.
#[expect(
    clippy::expect_used,
    reason = "Every translated user-error program has a known counterexample."
)]
fn benchmark_library<L, T>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    library: &str,
    property: fn(&T) -> bool,
) where
    L: Check<T> + Library,
    T: Debug,
{
    let _: &mut _ = group.bench_function(library, |bencher| {
        bencher.iter(|| black_box(L::check(property).expect("counterexample not found")));
    });
}

#[expect(
    clippy::absolute_paths,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "The bounded inputs make the faithful translation's indexing and arithmetic valid."
)]
mod bucketsort {
    fn buggy(arr: &[usize], k: usize) -> Vec<usize> {
        let mut counts = vec![0_usize; k];
        for &x in arr {
            counts[x] += 1;
        }
        let mut sorted = Vec::new();
        for (i, &count) in arr.iter().enumerate() {
            sorted.extend(core::iter::repeat_n(i, count));
        }
        sorted
    }

    fn correct(arr: &[usize], k: usize) -> Vec<usize> {
        let mut counts = vec![0_usize; k];
        for &x in arr {
            counts[x] += 1;
        }
        let mut sorted = Vec::new();
        for (i, count) in counts.into_iter().enumerate() {
            sorted.extend(core::iter::repeat_n(i, count));
        }
        sorted
    }

    pub(super) fn property((raw, raw_k): &(Vec<usize>, usize)) -> bool {
        let k = raw_k % 8 + 1;
        let arr: Vec<usize> = raw.iter().take(8).map(|x| x % k).collect();
        buggy(&arr, k) == correct(&arr, k)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::manual_is_multiple_of,
    clippy::missing_docs_in_private_items,
    clippy::trivially_copy_pass_by_ref,
    reason = "The arithmetic mirrors the source, and Check requires a borrowed input."
)]
mod get_factors {
    fn buggy(n: usize) -> Vec<usize> {
        if n == 1 {
            return vec![];
        }
        for i in 2..n {
            if i * i > n {
                break;
            }
            if n % i == 0 {
                let mut factors = vec![i];
                factors.extend(buggy(n / i));
                return factors;
            }
        }
        vec![]
    }

    fn correct(n: usize) -> Vec<usize> {
        if n == 1 {
            return vec![];
        }
        for i in 2..n {
            if i * i > n {
                break;
            }
            if n % i == 0 {
                let mut factors = vec![i];
                factors.extend(correct(n / i));
                return factors;
            }
        }
        vec![n]
    }

    pub(super) fn property(raw: &usize) -> bool {
        let n = raw % 200 + 1;
        buggy(n) == correct(n)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    reason = "The bounded peg numbers make the faithful translation's arithmetic valid."
)]
mod hanoi {
    fn buggy(height: usize, start: usize, end: usize) -> Vec<(usize, usize)> {
        let mut steps = Vec::new();
        if height > 0 {
            let helper = 6 - start - end;
            steps.extend(buggy(height - 1, start, helper));
            steps.push((start, helper));
            steps.extend(buggy(height - 1, helper, end));
        }
        steps
    }

    fn correct(height: usize, start: usize, end: usize) -> Vec<(usize, usize)> {
        let mut steps = Vec::new();
        if height > 0 {
            let helper = 6 - start - end;
            steps.extend(correct(height - 1, start, helper));
            steps.push((start, end));
            steps.extend(correct(height - 1, helper, end));
        }
        steps
    }

    pub(super) fn property(&(raw_height, raw_start, raw_end): &(usize, usize, usize)) -> bool {
        let height = raw_height % 6;
        let start = raw_start % 3 + 1;
        let mut end = raw_end % 3 + 1;
        if end == start {
            end = end % 3 + 1;
        }
        buggy(height, start, end) == correct(height, start, end)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::missing_docs_in_private_items,
    clippy::ptr_arg,
    reason = "The arithmetic is checked, and Check requires the concrete String input type."
)]
mod is_valid_parenthesization {
    fn buggy(parens: &str) -> bool {
        let mut depth = 0_usize;
        for paren in parens.chars() {
            if paren == '(' {
                depth += 1;
            } else if let Some(next) = depth.checked_sub(1) {
                depth = next;
            } else {
                return false;
            }
        }
        true
    }

    fn correct(parens: &str) -> bool {
        let mut depth = 0_usize;
        for paren in parens.chars() {
            if paren == '(' {
                depth += 1;
            } else if let Some(next) = depth.checked_sub(1) {
                depth = next;
            } else {
                return false;
            }
        }
        depth == 0
    }

    pub(super) fn property(raw: &String) -> bool {
        let parens: String = raw
            .chars()
            .take(16)
            .map(|ch| if ch <= '\u{7fff}' { '(' } else { ')' })
            .collect();
        buggy(&parens) == correct(&parens)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    clippy::std_instead_of_alloc,
    reason = "Normalization guarantees nonempty bounded slices and heaps."
)]
mod kheapsort {
    use {core::cmp::Reverse, std::collections::BinaryHeap};

    fn buggy(arr: &[usize], k: usize) -> Vec<usize> {
        let mut heap: BinaryHeap<Reverse<usize>> = arr[..k].iter().copied().map(Reverse).collect();
        let mut sorted = Vec::new();
        for &x in arr {
            heap.push(Reverse(x));
            sorted.push(heap.pop().expect("heap is nonempty").0);
        }
        sorted.extend(heap.into_sorted_vec().into_iter().rev().map(|x| x.0));
        sorted
    }

    fn correct(arr: &[usize], k: usize) -> Vec<usize> {
        let mut heap: BinaryHeap<Reverse<usize>> = arr[..k].iter().copied().map(Reverse).collect();
        let mut sorted = Vec::new();
        for &x in &arr[k..] {
            heap.push(Reverse(x));
            sorted.push(heap.pop().expect("heap is nonempty").0);
        }
        sorted.extend(heap.into_sorted_vec().into_iter().rev().map(|x| x.0));
        sorted
    }

    pub(super) fn property((raw, raw_k): &(Vec<usize>, usize)) -> bool {
        let mut arr: Vec<usize> = raw.iter().take(8).map(|x| x % 16).collect();
        arr.sort_unstable();
        arr.dedup();
        if arr.is_empty() {
            arr.push(0);
        }
        let k = raw_k % (arr.len() + 1);
        buggy(&arr, k) == correct(&arr, k)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "Bounded capacities and items make the faithful dynamic-programming indexes valid."
)]
mod knapsack {
    fn solve(capacity: usize, items: &[(usize, usize)], inclusive: bool) -> usize {
        let mut memo = vec![vec![0_usize; capacity + 1]; items.len() + 1];
        for i in 1..=items.len() {
            let (weight, value) = items[i - 1];
            for j in 1..=capacity {
                if if inclusive { weight <= j } else { weight < j } {
                    memo[i][j] = memo[i - 1][j].max(value + memo[i - 1][j - weight]);
                } else {
                    memo[i][j] = memo[i - 1][j];
                }
            }
        }
        memo[items.len()][capacity]
    }

    pub(super) fn property((raw_capacity, raw_items): &(usize, Vec<(usize, usize)>)) -> bool {
        let capacity = raw_capacity % 20 + 1;
        let items: Vec<(usize, usize)> = raw_items
            .iter()
            .take(6)
            .map(|&(weight, value)| (weight % 10 + 1, value % 20))
            .collect();
        solve(capacity, &items, false) == solve(capacity, &items, true)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "Normalization bounds the faithful selection translation's arithmetic."
)]
mod kth {
    fn buggy(arr: &[usize], k: usize) -> Option<usize> {
        let (&pivot, tail) = arr.split_first()?;
        let below: Vec<usize> = tail.iter().copied().filter(|x| *x < pivot).collect();
        let above: Vec<usize> = tail.iter().copied().filter(|x| *x > pivot).collect();
        let num_less_or_equal = arr.len() - above.len();
        if k < below.len() {
            buggy(&below, k)
        } else if k >= num_less_or_equal {
            buggy(&above, k)
        } else {
            Some(pivot)
        }
    }

    fn correct(arr: &[usize], k: usize) -> Option<usize> {
        let (&pivot, tail) = arr.split_first()?;
        let below: Vec<usize> = tail.iter().copied().filter(|x| *x < pivot).collect();
        let above: Vec<usize> = tail.iter().copied().filter(|x| *x > pivot).collect();
        let num_less_or_equal = arr.len() - above.len();
        if k < below.len() {
            correct(&below, k)
        } else if k >= num_less_or_equal {
            correct(&above, k - num_less_or_equal)
        } else {
            Some(pivot)
        }
    }

    pub(super) fn property((raw, raw_k): &(Vec<usize>, usize)) -> bool {
        let mut arr: Vec<usize> = raw.iter().take(12).map(|x| x % 20).collect();
        if arr.is_empty() {
            arr.push(0);
        }
        let k = raw_k % arr.len();
        buggy(&arr, k) == correct(&arr, k)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "Loop bounds establish every dynamic-programming index and subtraction."
)]
mod lcs_length {
    fn solve(s: &[char], t: &[char], diagonal: bool) -> usize {
        let mut dp = vec![vec![0_usize; t.len() + 1]; s.len() + 1];
        for i in 1..=s.len() {
            for j in 1..=t.len() {
                if s[i - 1] == t[j - 1] {
                    dp[i][j] = if diagonal {
                        dp[i - 1][j - 1] + 1
                    } else {
                        dp[i - 1][j] + 1
                    };
                }
            }
        }
        dp.into_iter().flatten().max().unwrap_or(0)
    }

    pub(super) fn property((raw_s, raw_t): &(String, String)) -> bool {
        let s: Vec<char> = raw_s
            .chars()
            .take(8)
            .map(|ch| if ch <= '\u{7fff}' { 'a' } else { 'b' })
            .collect();
        let t: Vec<char> = raw_t
            .chars()
            .take(8)
            .map(|ch| if ch <= '\u{7fff}' { 'a' } else { 'b' })
            .collect();
        solve(&s, &t, false) == solve(&s, &t, true)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "Nonempty branches establish every recursive slice and index."
)]
mod levenshtein {
    fn buggy(source: &[char], target: &[char]) -> usize {
        if source.is_empty() || target.is_empty() {
            return source.len().max(target.len());
        }
        if source[0] == target[0] {
            return 1 + buggy(&source[1..], &target[1..]);
        }
        1 + buggy(source, &target[1..])
            .min(buggy(&source[1..], &target[1..]))
            .min(buggy(&source[1..], target))
    }

    fn correct(source: &[char], target: &[char]) -> usize {
        if source.is_empty() || target.is_empty() {
            return source.len().max(target.len());
        }
        if source[0] == target[0] {
            return correct(&source[1..], &target[1..]);
        }
        1 + correct(source, &target[1..])
            .min(correct(&source[1..], &target[1..]))
            .min(correct(&source[1..], target))
    }

    pub(super) fn property((raw_source, raw_target): &(String, String)) -> bool {
        let source: Vec<char> = raw_source
            .chars()
            .take(5)
            .map(|ch| if ch <= '\u{7fff}' { 'a' } else { 'b' })
            .collect();
        let target: Vec<char> = raw_target
            .chars()
            .take(5)
            .map(|ch| if ch <= '\u{7fff}' { 'a' } else { 'b' })
            .collect();
        buggy(&source, &target) == correct(&source, &target)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::ptr_arg,
    reason = "The algorithm establishes its indexes, and Check requires Vec as the input type."
)]
mod lis {
    fn solve(arr: &[usize], preserve_longest: bool) -> usize {
        let mut ends: Vec<Option<usize>> = vec![None; arr.len() + 1];
        let mut longest = 0_usize;
        for (i, &value) in arr.iter().enumerate() {
            let length = (1..=longest)
                .filter(|&j| ends[j].is_some_and(|end| arr[end] < value))
                .max()
                .unwrap_or(0);
            if length == longest || ends[length + 1].is_some_and(|end| value < arr[end]) {
                ends[length + 1] = Some(i);
                longest = if preserve_longest {
                    longest.max(length + 1)
                } else {
                    length + 1
                };
            }
        }
        longest
    }

    pub(super) fn property(raw: &Vec<usize>) -> bool {
        let mut arr = Vec::new();
        for value in raw.iter().take(10).map(|x| x % 20) {
            if !arr.contains(&value) {
                arr.push(value);
            }
        }
        solve(&arr, false) == solve(&arr, true)
    }
}

#[expect(
    clippy::indexing_slicing,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "Nonempty branches establish every recursive slice and index."
)]
mod longest_common_subsequence {
    fn buggy(a: &[char], b: &[char]) -> String {
        if a.is_empty() || b.is_empty() {
            return String::new();
        }
        if a[0] == b[0] {
            let mut result = String::from(a[0]);
            result.push_str(&buggy(&a[1..], b));
            return result;
        }
        longer(buggy(a, &b[1..]), buggy(&a[1..], b))
    }

    fn correct(a: &[char], b: &[char]) -> String {
        if a.is_empty() || b.is_empty() {
            return String::new();
        }
        if a[0] == b[0] {
            let mut result = String::from(a[0]);
            result.push_str(&correct(&a[1..], &b[1..]));
            return result;
        }
        longer(correct(a, &b[1..]), correct(&a[1..], b))
    }

    fn longer(lhs: String, rhs: String) -> String {
        if lhs.chars().count() >= rhs.chars().count() {
            lhs
        } else {
            rhs
        }
    }

    pub(super) fn property((raw_a, raw_b): &(String, String)) -> bool {
        let a: Vec<char> = raw_a
            .chars()
            .take(7)
            .map(|ch| if ch <= '\u{7fff}' { 'a' } else { 'b' })
            .collect();
        let b: Vec<char> = raw_b
            .chars()
            .take(7)
            .map(|ch| if ch <= '\u{7fff}' { 'a' } else { 'b' })
            .collect();
        buggy(&a, &b) == correct(&a, &b)
    }
}

#[expect(
    clippy::absolute_paths,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::ptr_arg,
    reason = "Normalization supplies a nonempty palindrome and Check requires Vec input."
)]
mod next_palindrome {
    fn buggy(mut digits: Vec<usize>) -> Vec<usize> {
        let mut high_mid = digits.len() / 2;
        let mut low_mid = (digits.len() - 1) / 2;
        while high_mid < digits.len() {
            if digits[high_mid] == 9 {
                digits[high_mid] = 0;
                digits[low_mid] = 0;
                high_mid += 1;
                if low_mid == 0 {
                    break;
                }
                low_mid -= 1;
            } else {
                digits[high_mid] += 1;
                if low_mid != high_mid {
                    digits[low_mid] += 1;
                }
                return digits;
            }
        }
        let mut result = vec![1];
        result.extend(core::iter::repeat_n(0, digits.len()));
        result.push(1);
        result
    }

    fn correct(mut digits: Vec<usize>) -> Vec<usize> {
        let mut high_mid = digits.len() / 2;
        let mut low_mid = (digits.len() - 1) / 2;
        while high_mid < digits.len() {
            if digits[high_mid] == 9 {
                digits[high_mid] = 0;
                digits[low_mid] = 0;
                high_mid += 1;
                if low_mid == 0 {
                    break;
                }
                low_mid -= 1;
            } else {
                digits[high_mid] += 1;
                if low_mid != high_mid {
                    digits[low_mid] += 1;
                }
                return digits;
            }
        }
        let mut result = vec![1];
        result.extend(core::iter::repeat_n(0, digits.len().saturating_sub(1)));
        result.push(1);
        result
    }

    pub(super) fn property(raw: &Vec<usize>) -> bool {
        let mut digits: Vec<usize> = raw.iter().take(7).map(|x| x % 10).collect();
        if digits.is_empty() {
            digits.push(0);
        }
        for i in 0..(digits.len() / 2) {
            let opposite = digits.len() - i - 1;
            digits[opposite] = digits[i];
        }
        buggy(digits.clone()) == correct(digits)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_asserts_for_indexing,
    clippy::missing_docs_in_private_items,
    clippy::ptr_arg,
    reason = "Range bounds establish every index and Check requires Vec input."
)]
mod next_permutation {
    fn buggy(perm: &[usize]) -> Option<Vec<usize>> {
        for i in (0..perm.len().saturating_sub(1)).rev() {
            if perm[i] < perm[i + 1] {
                for j in ((i + 1)..perm.len()).rev() {
                    if perm[j] < perm[i] {
                        return Some(swapped(perm, i, j));
                    }
                }
            }
        }
        None
    }

    fn correct(perm: &[usize]) -> Option<Vec<usize>> {
        for i in (0..perm.len().saturating_sub(1)).rev() {
            if perm[i] < perm[i + 1] {
                for j in ((i + 1)..perm.len()).rev() {
                    if perm[j] > perm[i] {
                        return Some(swapped(perm, i, j));
                    }
                }
            }
        }
        None
    }

    pub(super) fn property(raw: &Vec<usize>) -> bool {
        let mut perm = Vec::new();
        for value in raw.iter().take(8).map(|x| x % 16) {
            if !perm.contains(&value) {
                perm.push(value);
            }
        }
        if perm.len() < 2 {
            perm = vec![0, 1];
        }
        if perm.windows(2).all(|pair| pair[0] > pair[1]) {
            perm.reverse();
        }
        buggy(&perm) == correct(&perm)
    }

    fn swapped(perm: &[usize], i: usize, j: usize) -> Vec<usize> {
        let mut result = perm.to_vec();
        result.swap(i, j);
        result[(i + 1)..].reverse();
        result
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::trivially_copy_pass_by_ref,
    reason = "Loop bounds establish every index, and Check requires a borrowed input."
)]
mod pascal {
    fn buggy(n: usize) -> Vec<Vec<usize>> {
        let mut rows = vec![vec![1_usize]];
        for r in 1..n {
            let mut row = Vec::new();
            for c in 0..r {
                let upper_left = if c > 0 { rows[r - 1][c - 1] } else { 0 };
                let upper_right = rows[r - 1].get(c).copied().unwrap_or(0);
                row.push(upper_left + upper_right);
            }
            rows.push(row);
        }
        rows
    }

    fn correct(n: usize) -> Vec<Vec<usize>> {
        let mut rows = vec![vec![1_usize]];
        for r in 1..n {
            let mut row = Vec::new();
            for c in 0..=r {
                let upper_left = if c > 0 { rows[r - 1][c - 1] } else { 0 };
                let upper_right = rows[r - 1].get(c).copied().unwrap_or(0);
                row.push(upper_left + upper_right);
            }
            rows.push(row);
        }
        rows
    }

    pub(super) fn property(raw: &usize) -> bool {
        let n = raw % 10 + 1;
        buggy(n) == correct(n)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_wrap,
    clippy::expect_used,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::pattern_type_mismatch,
    reason = "Normalization bounds all values before the faithful signed recursive translation."
)]
mod possible_change {
    fn buggy(coins: &[usize], total: isize) -> Result<usize, ()> {
        if total == 0 {
            return Ok(1);
        }
        if total < 0 {
            return Ok(0);
        }
        let Some((&first, rest)) = coins.split_first() else {
            return Err(());
        };
        Ok(buggy(coins, total - first as isize)? + buggy(rest, total)?)
    }

    fn correct(coins: &[usize], total: isize) -> usize {
        if total == 0 {
            return 1;
        }
        if total < 0 || coins.is_empty() {
            return 0;
        }
        let (first, rest) = coins.split_first().expect("coins are nonempty");
        correct(coins, total - *first as isize) + correct(rest, total)
    }

    pub(super) fn property((raw_coins, raw_total): &(Vec<usize>, usize)) -> bool {
        let mut coins: Vec<usize> = raw_coins.iter().take(5).map(|coin| coin % 8 + 1).collect();
        coins.sort_unstable();
        coins.dedup();
        let total = (raw_total % 20) as isize;
        buggy(&coins, total) == Ok(correct(&coins, total))
    }
}

#[expect(
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::ptr_arg,
    reason = "Normalization uses remainder and Check requires the concrete Vec input type."
)]
mod powerset {
    fn buggy(arr: &[usize]) -> Vec<Vec<usize>> {
        let Some((&first, rest)) = arr.split_first() else {
            return vec![vec![]];
        };
        buggy(rest)
            .into_iter()
            .map(|mut subset| {
                subset.insert(0, first);
                subset
            })
            .collect()
    }

    fn correct(arr: &[usize]) -> Vec<Vec<usize>> {
        let Some((&first, rest)) = arr.split_first() else {
            return vec![vec![]];
        };
        let rest_subsets = correct(rest);
        let mut result = rest_subsets.clone();
        result.extend(rest_subsets.into_iter().map(|mut subset| {
            subset.insert(0, first);
            subset
        }));
        result
    }

    pub(super) fn property(raw: &Vec<usize>) -> bool {
        let mut arr = Vec::new();
        for value in raw.iter().take(8).map(|x| x % 16) {
            if !arr.contains(&value) {
                arr.push(value);
            }
        }
        buggy(&arr) == correct(&arr)
    }
}

#[expect(
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::ptr_arg,
    reason = "Normalization uses remainder and Check requires the concrete Vec input type."
)]
mod quicksort {
    fn buggy(arr: &[usize]) -> Vec<usize> {
        let Some((&pivot, rest)) = arr.split_first() else {
            return vec![];
        };
        let mut result = buggy(
            &rest
                .iter()
                .copied()
                .filter(|x| *x < pivot)
                .collect::<Vec<_>>(),
        );
        result.push(pivot);
        result.extend(buggy(
            &rest
                .iter()
                .copied()
                .filter(|x| *x > pivot)
                .collect::<Vec<_>>(),
        ));
        result
    }

    fn correct(arr: &[usize]) -> Vec<usize> {
        let Some((&pivot, rest)) = arr.split_first() else {
            return vec![];
        };
        let mut result = correct(
            &rest
                .iter()
                .copied()
                .filter(|x| *x < pivot)
                .collect::<Vec<_>>(),
        );
        result.push(pivot);
        result.extend(correct(
            &rest
                .iter()
                .copied()
                .filter(|x| *x >= pivot)
                .collect::<Vec<_>>(),
        ));
        result
    }

    pub(super) fn property(raw: &Vec<usize>) -> bool {
        let arr: Vec<usize> = raw.iter().take(12).map(|x| x % 10).collect();
        buggy(&arr) == correct(&arr)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    clippy::trivially_copy_pass_by_ref,
    reason = "Divisibility is the algorithm, and Check requires a borrowed input."
)]
mod sieve {
    fn buggy(max: usize) -> Vec<usize> {
        let mut primes = Vec::new();
        for n in 2..=max {
            if primes.iter().any(|p| n % p > 0) {
                primes.push(n);
            }
        }
        primes
    }

    fn correct(max: usize) -> Vec<usize> {
        let mut primes = Vec::new();
        for n in 2..=max {
            if primes.iter().all(|p| n % p > 0) {
                primes.push(n);
            }
        }
        primes
    }

    pub(super) fn property(raw: &usize) -> bool {
        let max = raw % 100;
        buggy(max) == correct(max)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    reason = "Normalization bounds the faithful recursive range arithmetic."
)]
mod subsequences {
    fn buggy(a: usize, b: usize, k: usize) -> Vec<Vec<usize>> {
        if k == 0 {
            return vec![];
        }
        let mut result = Vec::new();
        for i in a..=b.saturating_add(1).saturating_sub(k) {
            for mut rest in buggy(i + 1, b, k - 1) {
                rest.insert(0, i);
                result.push(rest);
            }
        }
        result
    }

    fn correct(a: usize, b: usize, k: usize) -> Vec<Vec<usize>> {
        if k == 0 {
            return vec![vec![]];
        }
        let mut result = Vec::new();
        for i in a..=b.saturating_add(1).saturating_sub(k) {
            for mut rest in correct(i + 1, b, k - 1) {
                rest.insert(0, i);
                result.push(rest);
            }
        }
        result
    }

    pub(super) fn property(&(raw_a, raw_width, raw_k): &(usize, usize, usize)) -> bool {
        let a = raw_a % 6;
        let b = a + raw_width % 6;
        let k = raw_k % 4;
        buggy(a, b, k) == correct(a, b, k)
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::integer_division_remainder_used,
    clippy::missing_docs_in_private_items,
    reason = "Normalization bounds the base and every alphabet index."
)]
mod to_base {
    const ALPHABET: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    fn buggy(mut num: usize, base: usize) -> String {
        let mut result = String::new();
        while num > 0 {
            result.push(char::from(ALPHABET[num % base]));
            num /= base;
        }
        result
    }

    fn correct(mut num: usize, base: usize) -> String {
        let mut result = String::new();
        while num > 0 {
            result.insert(0, char::from(ALPHABET[num % base]));
            num /= base;
        }
        result
    }

    pub(super) fn property(&(raw_num, raw_base): &(usize, usize)) -> bool {
        let num = raw_num % 10_000 + 1;
        let base = raw_base % 35 + 2;
        buggy(num, base) == correct(num, base)
    }
}

/// Register the complete benchmark matrix.
fn quixbugs(criterion: &mut Criterion) {
    benchmark(criterion, "bucketsort", bucketsort::property);
    benchmark(criterion, "get_factors", get_factors::property);
    benchmark(criterion, "hanoi", hanoi::property);
    benchmark(
        criterion,
        "is_valid_parenthesization",
        is_valid_parenthesization::property,
    );
    benchmark(criterion, "kheapsort", kheapsort::property);
    benchmark(criterion, "knapsack", knapsack::property);
    benchmark(criterion, "kth", kth::property);
    benchmark(criterion, "lcs_length", lcs_length::property);
    benchmark(criterion, "levenshtein", levenshtein::property);
    benchmark(criterion, "lis", lis::property);
    benchmark(
        criterion,
        "longest_common_subsequence",
        longest_common_subsequence::property,
    );
    benchmark(criterion, "next_palindrome", next_palindrome::property);
    benchmark(criterion, "next_permutation", next_permutation::property);
    benchmark(criterion, "pascal", pascal::property);
    benchmark(criterion, "possible_change", possible_change::property);
    benchmark(criterion, "powerset", powerset::property);
    benchmark(criterion, "quicksort", quicksort::property);
    benchmark(criterion, "sieve", sieve::property);
    benchmark(criterion, "subsequences", subsequences::property);
    benchmark(criterion, "to_base", to_base::property);
}

criterion_group!(benches, quixbugs);
criterion_main!(benches);
