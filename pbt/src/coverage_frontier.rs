//! In-memory ranking and sampling for coverage-interesting candidates.
//!
//! Candidate ownership, exact novelty scoring, and cached frontier membership are
//! deliberately separate. The segment tree only stores immutable candidate handles and
//! finite nonnegative cached scores; it does not know how location novelty is calculated.
//! Integration must record a captured execution trace before admitting its candidate so
//! admission scores against the current visit counts.

use {
    alloc::{boxed::Box, sync::Arc, vec::Vec},
    core::{iter, mem},
};

/// A finite, nonnegative floating-point value.
///
/// Keeping this validation at construction sites prevents invalid multipliers, scores, and
/// aggregates from entering persistent module state.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FiniteNonnegative(f64);

impl FiniteNonnegative {
    /// Add two validated values, rejecting floating-point overflow.
    #[inline]
    #[expect(
        clippy::float_arithmetic,
        reason = "The result is immediately validated for floating-point overflow."
    )]
    fn add(self, rhs: Self, description: &str) -> Self {
        Self::new(self.0 + rhs.0, description)
    }

    /// Construct a validated finite, nonnegative value.
    #[inline]
    #[track_caller]
    fn new(value: f64, description: &str) -> Self {
        assert!(
            value.is_finite(),
            "INTERNAL ERROR (`pbt`): {description} must be finite, not {value:?}",
        );
        assert!(
            value >= 0.0_f64,
            "INTERNAL ERROR (`pbt`): {description} must be nonnegative, not {value:?}",
        );
        Self(value)
    }

    /// Compute this multiplier's novelty after `visits` executions.
    #[inline]
    #[expect(
        clippy::as_conversions,
        clippy::cast_precision_loss,
        clippy::float_arithmetic,
        reason = "The specified f64 scoring model necessarily converts visits and divides."
    )]
    fn novelty(self, visits: u64) -> Self {
        let denominator = visits as f64 + 1.0_f64;
        Self::new(self.0 / denominator, "location novelty")
    }

    /// Subtract a known-smaller value while descending the sampling tree.
    #[inline]
    #[expect(
        clippy::float_arithmetic,
        reason = "Tree descent proves the subtrahend is no greater than the remaining mass."
    )]
    fn subtract(self, rhs: Self) -> Self {
        assert!(
            self.0 >= rhs.0,
            "INTERNAL ERROR (`pbt`): sampling mass subtraction became negative",
        );
        Self::new(self.0 - rhs.0, "remaining sampling mass")
    }

    /// Access the validated floating-point value.
    #[inline]
    const fn value(self) -> f64 {
        self.0
    }

    /// The additive identity.
    #[inline]
    const fn zero() -> Self {
        Self(0.0)
    }
}

/// Scoring state for one physical coverage location.
#[derive(Clone, Copy, Debug)]
struct Location {
    /// The fixed campaign multiplier for this location.
    multiplier: FiniteNonnegative,
    /// Executions whose canonical trace contained this location.
    visits: u64,
}

impl Location {
    /// Construct an unvisited location with a validated multiplier.
    #[inline]
    fn new(multiplier: f64) -> Self {
        Self {
            multiplier: FiniteNonnegative::new(multiplier, "location multiplier"),
            visits: 0,
        }
    }

    /// Record one execution containing this location.
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "A visit count overflow is an impossible campaign state and must fail loudly."
    )]
    fn record_visit(&mut self) {
        self.visits = self
            .visits
            .checked_add(1)
            .expect("INTERNAL ERROR (`pbt`): coverage location visit count overflowed");
    }
}

/// An immutable generated value and its canonical sparse coverage trace.
struct Candidate<T> {
    /// The generated value owned independently of frontier membership.
    value: T,
    /// Sorted, duplicate-free physical location IDs visited by this candidate.
    visited_locations: Box<[u32]>,
}

impl<T> Candidate<T> {
    /// Construct a candidate from a trace promised to be sorted and duplicate-free.
    #[inline]
    fn new(value: T, visited_locations: Box<[u32]>) -> Self {
        assert_canonical_trace(&visited_locations);
        Self {
            value,
            visited_locations,
        }
    }
}

/// One active stable leaf in the frontier.
struct FrontierSlot<T> {
    /// An upper bound on the candidate's current exact score.
    cached_score: FiniteNonnegative,
    /// The candidate whose ownership may also be shared by future indexes.
    candidate: Arc<Candidate<T>>,
}

/// A cached subtree minimum and the stable slot attaining it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Minimum {
    /// The minimum active cached score.
    score: FiniteNonnegative,
    /// The lowest stable slot attaining `score`.
    slot: usize,
}

/// Sum and minimum aggregates for one complete segment-tree node.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Aggregate {
    /// The minimum active cached score, excluding inactive leaves.
    minimum: Option<Minimum>,
    /// The total cached sampling weight in this subtree.
    sum: FiniteNonnegative,
}

impl Aggregate {
    /// Construct an active leaf aggregate.
    #[inline]
    const fn active(score: FiniteNonnegative, slot: usize) -> Self {
        Self {
            minimum: Some(Minimum { score, slot }),
            sum: score,
        }
    }

    /// Combine two child aggregates.
    #[inline]
    fn combine(left: Self, right: Self) -> Self {
        let minimum = match (left.minimum, right.minimum) {
            (None, None) => None,
            (Some(minimum), None) | (None, Some(minimum)) => Some(minimum),
            (Some(left_minimum), Some(right_minimum)) => {
                // Every slot in the left subtree precedes every slot in the right subtree,
                // so equal scores keep the left minimum.
                let right_is_lower = right_minimum.score.value() < left_minimum.score.value();
                Some(if right_is_lower {
                    right_minimum
                } else {
                    left_minimum
                })
            }
        };
        Self {
            minimum,
            sum: left.sum.add(right.sum, "frontier subtree sum"),
        }
    }

    /// Construct an inactive leaf aggregate.
    #[inline]
    const fn inactive() -> Self {
        Self {
            minimum: None,
            sum: FiniteNonnegative::zero(),
        }
    }
}

/// A fixed-capacity frontier backed by one complete array-based segment tree.
///
/// `aggregates` stores every complete-tree node, while `slots` stores membership only for
/// logical leaves. Stable slot `i` corresponds to aggregate leaf `leaf_start + i`; aggregate
/// leaves beyond `logical_capacity` are permanent padding.
struct Frontier<T> {
    /// Number of currently active logical leaves.
    active_len: usize,
    /// Complete segment-tree aggregates in breadth-first array order.
    aggregates: Box<[Aggregate]>,
    /// LIFO set of inactive logical slots, arranged to allocate low slots first initially.
    free_slots: Vec<usize>,
    /// Number of leaves in the complete, power-of-two tree.
    leaf_capacity: usize,
    /// Array index of stable logical slot zero.
    leaf_start: usize,
    /// Requested capacity, excluding permanently inactive padding.
    logical_capacity: usize,
    /// Candidate membership for logical leaves only.
    slots: Box<[Option<FrontierSlot<T>>]>,
}

#[expect(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    reason = "Checked construction makes complete-tree index arithmetic an internal invariant."
)]
impl<T> Frontier<T> {
    /// Return the active stable slot at `slot`.
    #[inline]
    fn active_slot(&self, slot: usize) -> &FrontierSlot<T> {
        self.slots
            .get(slot)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier slot is out of range")
            .as_ref()
            .expect("INTERNAL ERROR (`pbt`): requested coverage frontier slot is inactive")
    }

    /// Access a tree aggregate, asserting the complete-tree index invariant.
    #[inline]
    fn aggregate(&self, node_index: usize) -> &Aggregate {
        self.aggregates
            .get(node_index)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier tree index is out of range")
    }

    /// Mutably access a tree aggregate, asserting the complete-tree index invariant.
    #[inline]
    fn aggregate_mut(&mut self, node_index: usize) -> &mut Aggregate {
        self.aggregates
            .get_mut(node_index)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier tree index is out of range")
    }

    /// Insert a candidate into the next free stable leaf.
    ///
    /// This performs one leaf-to-root repair and therefore takes `O(log N)` time.
    #[inline]
    fn insert(&mut self, candidate: Arc<Candidate<T>>, exact_score: f64) -> usize {
        let cached_score = FiniteNonnegative::new(exact_score, "candidate score");
        assert!(
            self.active_len < self.logical_capacity,
            "INTERNAL ERROR (`pbt`): inserting into a full coverage frontier",
        );
        let slot = self
            .free_slots
            .pop()
            .expect("INTERNAL ERROR (`pbt`): non-full frontier has no free slot");
        let leaf_index = self.leaf_index(slot);
        assert!(
            self.slots
                .get(slot)
                .expect("INTERNAL ERROR (`pbt`): free slot is out of range")
                .is_none(),
            "INTERNAL ERROR (`pbt`): free coverage frontier slot is active",
        );
        *self
            .slots
            .get_mut(slot)
            .expect("INTERNAL ERROR (`pbt`): free slot disappeared") = Some(FrontierSlot {
            cached_score,
            candidate,
        });
        *self.aggregate_mut(leaf_index) = Aggregate::active(cached_score, slot);
        self.active_len = self
            .active_len
            .checked_add(1)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier length overflowed");
        self.repair_from(leaf_index);
        slot
    }

    /// Convert a stable logical slot into its leaf array index.
    #[inline]
    fn leaf_index(&self, slot: usize) -> usize {
        assert!(
            slot < self.logical_capacity,
            "INTERNAL ERROR (`pbt`): coverage frontier slot is out of range",
        );
        self.leaf_start + slot
    }

    /// Return the minimum active cached score and its deterministic victim slot.
    #[inline]
    fn minimum_cached_score(&self) -> Option<(f64, usize)> {
        self.root()
            .minimum
            .map(|minimum| (minimum.score.value(), minimum.slot))
    }

    /// Construct an empty frontier with a nonzero fixed logical capacity.
    ///
    /// Construction allocates and initializes the next complete power-of-two tree, taking
    /// `O(next_power_of_two(N))` time and space.
    #[inline]
    fn new(logical_capacity: usize) -> Self {
        assert!(
            logical_capacity > 0,
            "INTERNAL ERROR (`pbt`): coverage frontier capacity must be nonzero",
        );
        let leaf_capacity = logical_capacity
            .checked_next_power_of_two()
            .expect("INTERNAL ERROR (`pbt`): coverage frontier capacity is too large");
        let leaf_start = leaf_capacity
            .checked_sub(1)
            .expect("INTERNAL ERROR (`pbt`): nonzero tree has no leaves");
        let node_count = leaf_capacity
            .checked_mul(2)
            .and_then(|twice| twice.checked_sub(1))
            .expect("INTERNAL ERROR (`pbt`): coverage frontier tree is too large");
        let aggregates = iter::repeat_n(Aggregate::inactive(), node_count).collect();
        let slots = iter::repeat_with(|| None).take(logical_capacity).collect();
        let free_slots = (0..logical_capacity).rev().collect();
        Self {
            active_len: 0,
            aggregates,
            free_slots,
            leaf_capacity,
            leaf_start,
            logical_capacity,
            slots,
        }
    }

    /// Select the active stable leaf containing `sampling_mass`.
    ///
    /// `sampling_mass` must belong to `[0, total_cached_score())`. This performs exactly one
    /// root-to-leaf traversal and therefore takes `O(log N)` time.
    #[inline]
    fn propose(&self, sampling_mass: f64) -> usize {
        let mut remaining =
            FiniteNonnegative::new(sampling_mass, "coverage frontier sampling mass");
        let total = self.root().sum;
        assert!(
            remaining.value() < total.value(),
            "INTERNAL ERROR (`pbt`): sampling mass must be below total cached score",
        );

        let mut node_index = 0;
        while node_index < self.leaf_start {
            let (left_index, right_index) = child_indices(node_index);
            let left_sum = self.aggregate(left_index).sum;
            if remaining.value() < left_sum.value() {
                node_index = left_index;
            } else {
                remaining = remaining.subtract(left_sum);
                node_index = right_index;
            }
        }

        let slot = node_index
            .checked_sub(self.leaf_start)
            .expect("INTERNAL ERROR (`pbt`): proposal did not reach a leaf");
        assert!(
            slot < self.logical_capacity,
            "INTERNAL ERROR (`pbt`): proposal selected a padded leaf",
        );
        assert!(
            self.active_slot(slot).cached_score.value() > 0.0_f64,
            "INTERNAL ERROR (`pbt`): proposal selected a zero-weight leaf",
        );
        slot
    }

    /// Remove an active candidate and return its shared ownership handle.
    ///
    /// This performs one leaf-to-root repair and therefore takes `O(log N)` time.
    #[inline]
    fn remove(&mut self, slot: usize) -> Arc<Candidate<T>> {
        let leaf_index = self.leaf_index(slot);
        let removed = self
            .slots
            .get_mut(slot)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier slot is out of range")
            .take()
            .expect("INTERNAL ERROR (`pbt`): removing an inactive coverage frontier slot");
        *self.aggregate_mut(leaf_index) = Aggregate::inactive();
        self.active_len = self
            .active_len
            .checked_sub(1)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier length underflowed");
        self.free_slots.push(slot);
        self.repair_from(leaf_index);
        removed.candidate
    }

    /// Repair all ancestors of one changed leaf.
    #[inline]
    fn repair_from(&mut self, mut node_index: usize) {
        while node_index > 0 {
            let ancestor_index = parent_index(node_index);
            let (left_index, right_index) = child_indices(ancestor_index);
            let aggregate =
                Aggregate::combine(*self.aggregate(left_index), *self.aggregate(right_index));
            *self.aggregate_mut(ancestor_index) = aggregate;
            node_index = ancestor_index;
        }
    }

    /// Replace an active stable leaf in place and return the evicted candidate.
    ///
    /// This performs one leaf-to-root repair and therefore takes `O(log N)` time.
    #[inline]
    fn replace(
        &mut self,
        slot: usize,
        candidate: Arc<Candidate<T>>,
        exact_score: f64,
    ) -> Arc<Candidate<T>> {
        let cached_score = FiniteNonnegative::new(exact_score, "candidate score");
        let leaf_index = self.leaf_index(slot);
        let evicted = mem::replace(
            self.slots
                .get_mut(slot)
                .expect("INTERNAL ERROR (`pbt`): coverage frontier slot is out of range")
                .as_mut()
                .expect("INTERNAL ERROR (`pbt`): replacing an inactive coverage frontier slot"),
            FrontierSlot {
                cached_score,
                candidate,
            },
        );
        *self.aggregate_mut(leaf_index) = Aggregate::active(cached_score, slot);
        self.repair_from(leaf_index);
        evicted.candidate
    }

    /// Access the root node.
    #[inline]
    fn root(&self) -> &Aggregate {
        self.aggregate(0)
    }

    /// Return the total cached sampling weight.
    #[inline]
    fn total_cached_score(&self) -> f64 {
        self.root().sum.value()
    }

    /// Decrease one active candidate's cached score and repair its ancestors.
    ///
    /// Score increases violate the campaign's monotone visit-count model and fail loudly.
    /// This performs one leaf-to-root repair and therefore takes `O(log N)` time.
    #[inline]
    fn update_score(&mut self, slot: usize, exact_score: f64) {
        let updated_score = FiniteNonnegative::new(exact_score, "candidate score");
        let previous_score = self.active_slot(slot).cached_score;
        assert!(
            updated_score.value() <= previous_score.value(),
            "INTERNAL ERROR (`pbt`): exact coverage score increased during recomputation",
        );

        let leaf_index = self.leaf_index(slot);
        self.slots
            .get_mut(slot)
            .expect("INTERNAL ERROR (`pbt`): coverage frontier slot is out of range")
            .as_mut()
            .expect("INTERNAL ERROR (`pbt`): updating an inactive coverage frontier slot")
            .cached_score = updated_score;
        *self.aggregate_mut(leaf_index) = Aggregate::active(updated_score, slot);
        self.repair_from(leaf_index);
    }
}

/// The ownership result of attempting to admit one candidate.
enum Admission<T> {
    /// The frontier had a free stable slot.
    Inserted {
        /// Stable slot assigned to the new candidate.
        slot: usize,
    },
    /// The candidate was not strictly better than the selected victim.
    Rejected {
        /// Ownership returned to the caller.
        candidate: Arc<Candidate<T>>,
    },
    /// A strictly worse candidate was evicted.
    Replaced {
        /// Shared ownership of the evicted candidate.
        evicted: Arc<Candidate<T>>,
        /// Stable slot retained by the replacement.
        slot: usize,
    },
}

/// Deterministic random values for one lazy rejection-sampling attempt.
struct SamplingDraw {
    /// A uniform acceptance value in `[0, 1)`.
    acceptance_value: f64,
    /// A mass in `[0, current_total_cached_score)`.
    proposal_mass: f64,
}

/// Admit one candidate after its execution trace has been recorded in `locations`.
///
/// Callers must capture and validate the candidate's trace, pass it to [`record_trace`], and
/// only then call this function. This ordering makes the new candidate's score exact under the
/// same current location state used for the cached-minimum replacement proof.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "A full frontier must have an active minimum by construction."
)]
fn admit_after_recording_trace<T>(
    frontier: &mut Frontier<T>,
    candidate: Arc<Candidate<T>>,
    locations: &[Location],
) -> Admission<T> {
    let candidate_score = exact_score(&candidate, locations);
    if frontier.active_len < frontier.logical_capacity {
        let slot = frontier.insert(candidate, candidate_score);
        return Admission::Inserted { slot };
    }

    let (cached_minimum, victim_slot) = frontier
        .minimum_cached_score()
        .expect("INTERNAL ERROR (`pbt`): full coverage frontier has no minimum");
    if candidate_score > cached_minimum {
        let evicted = frontier.replace(victim_slot, candidate, candidate_score);
        return Admission::Replaced {
            evicted,
            slot: victim_slot,
        };
    }

    let victim = Arc::clone(&frontier.active_slot(victim_slot).candidate);
    let exact_victim_score = exact_score(&victim, locations);
    frontier.update_score(victim_slot, exact_victim_score);
    if candidate_score > exact_victim_score {
        let evicted = frontier.replace(victim_slot, candidate, candidate_score);
        Admission::Replaced {
            evicted,
            slot: victim_slot,
        }
    } else {
        Admission::Rejected { candidate }
    }
}

/// Validate the sparse trace representation shared by candidates and executions.
#[inline]
#[track_caller]
fn assert_canonical_trace(visited_locations: &[u32]) {
    let mut locations = visited_locations.iter().copied();
    let Some(mut previous) = locations.next() else {
        return;
    };
    for location in locations {
        assert!(
            previous < location,
            "INTERNAL ERROR (`pbt`): coverage traces must be sorted and duplicate-free",
        );
        previous = location;
    }
}

/// Return the two child indices of one internal node in breadth-first array order.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Frontier construction proves every traversed internal node has two allocated children."
)]
fn child_indices(parent_index: usize) -> (usize, usize) {
    let left_index = parent_index * 2 + 1;
    (left_index, left_index + 1)
}

/// Compute a candidate's exact score against the current location table.
///
/// The candidate trace is sparse, so this operation is linear in the number of visited
/// locations rather than the total number of instrumented locations.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "A candidate referencing an unknown location is an invalid campaign state."
)]
fn exact_score<T>(candidate: &Candidate<T>, locations: &[Location]) -> f64 {
    let mut score = FiniteNonnegative::zero();
    for &location_id in &candidate.visited_locations {
        let location_index = usize::try_from(location_id)
            .expect("INTERNAL ERROR (`pbt`): coverage location ID does not fit usize");
        let location = locations
            .get(location_index)
            .expect("INTERNAL ERROR (`pbt`): coverage location ID is out of range");
        score = score.add(
            location.multiplier.novelty(location.visits),
            "candidate score",
        );
    }
    score.value()
}

/// Return one non-root node's parent index in breadth-first array order.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    reason = "The caller proves the node is non-root and complete-tree parent arithmetic is exact."
)]
fn parent_index(node_index: usize) -> usize {
    assert!(
        node_index > 0,
        "INTERNAL ERROR (`pbt`): the coverage frontier root has no parent",
    );
    (node_index - 1) / 2
}

/// Increment every location in one canonical execution trace exactly once.
///
/// The complete trace is validated before any visit count is mutated.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Out-of-range IDs and visit overflow are invalid campaign states."
)]
fn record_trace(visited_locations: &[u32], locations: &mut [Location]) {
    assert_canonical_trace(visited_locations);

    for &location_id in visited_locations {
        let location_index = usize::try_from(location_id)
            .expect("INTERNAL ERROR (`pbt`): coverage location ID does not fit usize");
        let location = locations
            .get(location_index)
            .expect("INTERNAL ERROR (`pbt`): coverage location ID is out of range");
        let _: u64 = location
            .visits
            .checked_add(1)
            .expect("INTERNAL ERROR (`pbt`): coverage location visit count overflowed");
    }

    for &location_id in visited_locations {
        let location_index = usize::try_from(location_id)
            .expect("INTERNAL ERROR (`pbt`): coverage location ID does not fit usize");
        locations
            .get_mut(location_index)
            .expect("INTERNAL ERROR (`pbt`): coverage location disappeared")
            .record_visit();
    }
}

/// Lazily repair stale scores while rejection-sampling an exactly weighted candidate.
///
/// The draw callback receives the current total before each attempt. Keeping random-value
/// generation outside this function makes every proposal and acceptance decision deterministic
/// under test.
#[inline]
#[expect(
    clippy::float_arithmetic,
    reason = "The acceptance ratio is the specified exact score divided by positive cached score."
)]
fn sample_with_draws<T, Draw>(
    frontier: &mut Frontier<T>,
    locations: &[Location],
    draw: &mut Draw,
) -> Option<Arc<Candidate<T>>>
where
    Draw: FnMut(f64) -> SamplingDraw,
{
    loop {
        let total_cached_score = frontier.total_cached_score();
        if total_cached_score <= 0.0_f64 {
            return None;
        }

        let SamplingDraw {
            acceptance_value,
            proposal_mass,
        } = draw(total_cached_score);
        let validated_acceptance_value =
            FiniteNonnegative::new(acceptance_value, "sampling acceptance value");
        assert!(
            validated_acceptance_value.value() < 1.0_f64,
            "INTERNAL ERROR (`pbt`): sampling acceptance value must be below one",
        );

        let proposed_slot = frontier.propose(proposal_mass);
        let proposed = frontier.active_slot(proposed_slot);
        let candidate = Arc::clone(&proposed.candidate);
        let cached_score = proposed.cached_score.value();
        assert!(
            cached_score > 0.0_f64,
            "INTERNAL ERROR (`pbt`): proposed candidate has zero cached score",
        );

        let corrected_score = exact_score(&candidate, locations);
        assert!(
            corrected_score <= cached_score,
            "INTERNAL ERROR (`pbt`): exact coverage score increased during recomputation",
        );
        let acceptance_probability = corrected_score / cached_score;
        let accepted = validated_acceptance_value.value() < acceptance_probability;
        frontier.update_score(proposed_slot, corrected_score);
        if accepted {
            return Some(candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::arithmetic_side_effects,
        reason = "Small deterministic tree indices and exact test scores cannot overflow."
    )]
    #![expect(
        clippy::integer_division_remainder_used,
        reason = "Generated hints are normalized into nonempty model collections."
    )]
    #![expect(
        clippy::assertions_on_result_states,
        clippy::default_numeric_fallback,
        clippy::expect_used,
        clippy::float_cmp,
        clippy::panic,
        clippy::unwrap_used,
        reason = "Deterministic tests use exact representable values and panic on wrong variants."
    )]

    use {
        super::*,
        crate::{Pbt, pbt},
        alloc::collections::{BTreeSet, VecDeque},
        core::panic::AssertUnwindSafe,
        pretty_assertions::assert_eq,
        std::panic::catch_unwind,
    };

    /// One generated state-machine command.
    #[derive(Clone, Copy, Debug, Pbt)]
    enum ModelOperation {
        /// Lower one active cached score.
        Decrease { score_cap: u8, slot_hint: usize },
        /// Insert into a production-selected free slot.
        Insert { score: u8 },
        /// Compare weighted selection with a cumulative scan.
        Propose { mass_hint: usize },
        /// Remove one active slot.
        Remove { slot_hint: usize },
        /// Replace one active slot in place.
        Replace { score: u8, slot_hint: usize },
    }

    /// A generated logical capacity and operation sequence.
    #[derive(Clone, Debug, Pbt)]
    struct ModelScenario {
        /// Normalized into the inclusive range `1..=17`.
        capacity_hint: u8,
        /// Commands interpreted without filtering.
        operations: Vec<ModelOperation>,
    }

    /// Straightforward reference state for one stable logical slot.
    #[derive(Clone, Copy, Debug)]
    struct ModelSlot {
        /// Operation index that created this membership.
        candidate_identity: usize,
        /// Small integer-valued score represented exactly by `f64`.
        score: f64,
    }

    /// Build an immutable test candidate.
    fn candidate<T>(value: T, visited_locations: &[u32]) -> Arc<Candidate<T>> {
        Arc::new(Candidate::new(
            value,
            visited_locations.to_vec().into_boxed_slice(),
        ))
    }

    /// Build location state with a known visit count.
    fn location(multiplier: f64, visits: u64) -> Location {
        let mut location = Location::new(multiplier);
        location.visits = visits;
        location
    }

    /// Recompute and verify every tree aggregate and membership invariant.
    fn assert_invariants<T>(frontier: &Frontier<T>) {
        fn verify_node<T>(frontier: &Frontier<T>, node_index: usize) -> Aggregate {
            let actual = *frontier.aggregate(node_index);
            let expected = if node_index < frontier.leaf_start {
                let left_index = node_index * 2 + 1;
                Aggregate::combine(
                    verify_node(frontier, left_index),
                    verify_node(frontier, left_index + 1),
                )
            } else {
                let slot = node_index - frontier.leaf_start;
                if slot < frontier.logical_capacity {
                    frontier
                        .slots
                        .get(slot)
                        .expect("logical slot should exist")
                        .as_ref()
                        .map_or_else(Aggregate::inactive, |frontier_slot| {
                            Aggregate::active(frontier_slot.cached_score, slot)
                        })
                } else {
                    Aggregate::inactive()
                }
            };
            assert_eq!(actual, expected);
            expected
        }

        assert_eq!(
            frontier.aggregates.len(),
            frontier.leaf_capacity * 2 - 1,
            "tree must be complete",
        );
        assert_eq!(
            frontier.slots.len(),
            frontier.logical_capacity,
            "only logical leaves store membership",
        );
        let recursively_verified = verify_node(frontier, 0);

        let mut scanned = Aggregate::inactive();
        let mut scanned_active_len = 0;
        for (slot, membership) in frontier.slots.iter().enumerate() {
            if let Some(frontier_slot) = membership.as_ref() {
                scanned = Aggregate::combine(
                    scanned,
                    Aggregate::active(frontier_slot.cached_score, slot),
                );
                scanned_active_len += 1;
            }
        }
        assert_eq!(*frontier.root(), scanned);
        assert_eq!(*frontier.root(), recursively_verified);
        assert_eq!(frontier.active_len, scanned_active_len);

        let free_slots: BTreeSet<usize> = frontier.free_slots.iter().copied().collect();
        assert_eq!(
            free_slots.len(),
            frontier.free_slots.len(),
            "free-slot stack must not contain duplicates",
        );
        for slot in 0..frontier.logical_capacity {
            assert_eq!(
                free_slots.contains(&slot),
                frontier
                    .slots
                    .get(slot)
                    .expect("logical slot should exist")
                    .is_none(),
            );
        }
    }

    /// Compare every semantic frontier state component with the reference model.
    fn assert_matches_model(frontier: &Frontier<usize>, model: &[Option<ModelSlot>]) {
        assert_eq!(frontier.logical_capacity, model.len());
        assert_eq!(
            frontier.active_len,
            model
                .iter()
                .filter(|membership| membership.is_some())
                .count(),
        );

        for (slot, expected_membership) in model.iter().enumerate() {
            let actual_membership = frontier.slots.get(slot).expect("logical slot should exist");
            match (actual_membership.as_ref(), expected_membership.as_ref()) {
                (None, None) => {}
                (Some(actual), Some(expected)) => {
                    assert_eq!(actual.candidate.value, expected.candidate_identity);
                    assert_eq!(actual.cached_score.value(), expected.score);
                }
                (None, Some(_)) | (Some(_), None) => {
                    panic!("production and model membership disagree at slot {slot}")
                }
            }
        }

        let expected_total = model
            .iter()
            .filter_map(Option::as_ref)
            .map(|slot| slot.score)
            .sum::<f64>();
        assert_eq!(frontier.total_cached_score(), expected_total);

        let expected_minimum = model
            .iter()
            .enumerate()
            .filter_map(|(slot, membership)| membership.as_ref().map(|entry| (entry.score, slot)))
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            });
        assert_eq!(frontier.minimum_cached_score(), expected_minimum);

        let expected_free_slots: BTreeSet<usize> = model
            .iter()
            .enumerate()
            .filter_map(|(slot, membership)| membership.is_none().then_some(slot))
            .collect();
        let actual_free_slots: BTreeSet<usize> = frontier.free_slots.iter().copied().collect();
        assert_eq!(actual_free_slots, expected_free_slots);
    }

    /// Select an active model slot by normalizing a generated hint.
    fn model_active_slot(model: &[Option<ModelSlot>], slot_hint: usize) -> Option<usize> {
        let active_slots: Vec<usize> = model
            .iter()
            .enumerate()
            .filter_map(|(slot, membership)| membership.is_some().then_some(slot))
            .collect();
        if active_slots.is_empty() {
            return None;
        }
        active_slots.get(slot_hint % active_slots.len()).copied()
    }

    /// Select a positive-weight slot by a naive left-to-right cumulative scan.
    #[expect(
        clippy::float_arithmetic,
        reason = "All model scores and masses are small integers represented exactly by f64."
    )]
    fn model_proposal(model: &[Option<ModelSlot>], mass: f64) -> usize {
        let mut cumulative = 0.0_f64;
        for (slot, membership) in model.iter().enumerate() {
            if let Some(entry) = membership.as_ref() {
                cumulative += entry.score;
                if mass < cumulative {
                    return slot;
                }
            }
        }
        panic!("valid model proposal mass did not select an active slot")
    }

    #[test]
    fn aggregate_minimum_prefers_lower_score_then_left_slot() {
        let left_lower = Aggregate::combine(
            Aggregate::active(FiniteNonnegative::new(1.0, "test score"), 0),
            Aggregate::active(FiniteNonnegative::new(2.0, "test score"), 1),
        );
        assert_eq!(
            left_lower.minimum,
            Some(Minimum {
                score: FiniteNonnegative::new(1.0, "test score"),
                slot: 0,
            }),
        );

        let right_lower = Aggregate::combine(
            Aggregate::active(FiniteNonnegative::new(2.0, "test score"), 0),
            Aggregate::active(FiniteNonnegative::new(1.0, "test score"), 1),
        );
        assert_eq!(
            right_lower.minimum,
            Some(Minimum {
                score: FiniteNonnegative::new(1.0, "test score"),
                slot: 1,
            }),
        );

        let equal = Aggregate::combine(
            Aggregate::active(FiniteNonnegative::new(1.0, "test score"), 0),
            Aggregate::active(FiniteNonnegative::new(1.0, "test score"), 1),
        );
        assert_eq!(
            equal.minimum,
            Some(Minimum {
                score: FiniteNonnegative::new(1.0, "test score"),
                slot: 0,
            }),
        );
    }

    #[test]
    fn complete_tree_index_arithmetic_is_exact() {
        assert_eq!(child_indices(0), (1, 2));
        assert_eq!(child_indices(1), (3, 4));
        assert_eq!(child_indices(2), (5, 6));

        assert_eq!(parent_index(1), 0);
        assert_eq!(parent_index(2), 0);
        assert_eq!(parent_index(3), 1);
        assert_eq!(parent_index(4), 1);
    }

    #[pbt]
    fn frontier_matches_reference_model(scenario: &ModelScenario) {
        let logical_capacity = usize::from(scenario.capacity_hint % 17) + 1;
        let mut frontier = Frontier::new(logical_capacity);
        let mut model = vec![None; logical_capacity];
        assert_invariants(&frontier);
        assert_matches_model(&frontier, &model);

        for (operation_index, operation) in scenario.operations.iter().enumerate() {
            match *operation {
                ModelOperation::Decrease {
                    score_cap,
                    slot_hint,
                } => {
                    if let Some(slot) = model_active_slot(&model, slot_hint) {
                        let current_score = model
                            .get(slot)
                            .and_then(Option::as_ref)
                            .expect("selected model slot should be active")
                            .score;
                        let decreased_score = current_score.min(f64::from(score_cap));
                        frontier.update_score(slot, decreased_score);
                        model
                            .get_mut(slot)
                            .and_then(Option::as_mut)
                            .expect("selected model slot should remain active")
                            .score = decreased_score;
                    }
                }
                ModelOperation::Insert { score } => {
                    if model.iter().any(Option::is_none) {
                        let exact_score = f64::from(score);
                        let slot = frontier.insert(candidate(operation_index, &[]), exact_score);
                        let membership = model
                            .get_mut(slot)
                            .expect("production selected an unknown slot");
                        assert!(
                            membership.is_none(),
                            "production insertion must select a model-free slot",
                        );
                        *membership = Some(ModelSlot {
                            candidate_identity: operation_index,
                            score: exact_score,
                        });
                    }
                }
                ModelOperation::Propose { mass_hint } => {
                    let positive_slots: Vec<usize> = model
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, membership)| {
                            membership
                                .as_ref()
                                .is_some_and(|entry| entry.score > 0.0_f64)
                                .then_some(slot)
                        })
                        .collect();
                    if !positive_slots.is_empty() {
                        let target_slot = *positive_slots
                            .get(mass_hint % positive_slots.len())
                            .expect("normalized positive slot should exist");
                        let mass = model
                            .iter()
                            .take(target_slot)
                            .filter_map(Option::as_ref)
                            .map(|entry| entry.score)
                            .sum::<f64>();
                        assert_eq!(model_proposal(&model, mass), target_slot);
                        assert_eq!(frontier.propose(mass), target_slot);
                    }
                }
                ModelOperation::Remove { slot_hint } => {
                    if let Some(slot) = model_active_slot(&model, slot_hint) {
                        let expected = model
                            .get_mut(slot)
                            .expect("selected model slot should exist")
                            .take()
                            .expect("selected model slot should be active");
                        let removed = frontier.remove(slot);
                        assert_eq!(removed.value, expected.candidate_identity);
                    }
                }
                ModelOperation::Replace { score, slot_hint } => {
                    if let Some(slot) = model_active_slot(&model, slot_hint) {
                        let exact_score = f64::from(score);
                        let evicted =
                            frontier.replace(slot, candidate(operation_index, &[]), exact_score);
                        let replaced = mem::replace(
                            model
                                .get_mut(slot)
                                .and_then(Option::as_mut)
                                .expect("selected model slot should be active"),
                            ModelSlot {
                                candidate_identity: operation_index,
                                score: exact_score,
                            },
                        );
                        assert_eq!(evicted.value, replaced.candidate_identity);
                    }
                }
            }

            assert_invariants(&frontier);
            assert_matches_model(&frontier, &model);
        }
    }

    #[test]
    fn tree_shapes_and_aggregates_survive_every_operation() {
        let scores = [1.0, 2.0, 3.0, 4.0, 5.0];
        for capacity in [1, 2, 3, 4, 5] {
            let mut frontier = Frontier::new(capacity);
            assert_eq!(frontier.leaf_capacity, capacity.next_power_of_two());
            assert_invariants(&frontier);

            let mut slots = Vec::new();
            for (value, &score) in scores.iter().take(capacity).enumerate() {
                slots.push(frontier.insert(candidate(value, &[]), score));
                assert_invariants(&frontier);
            }
            assert_eq!(slots, (0..capacity).collect::<Vec<usize>>());

            while let Some(slot) = slots.pop() {
                let removed = frontier.remove(slot);
                assert_eq!(removed.value, slot);
                assert_invariants(&frontier);
            }
        }
    }

    #[test]
    fn weighted_proposal_obeys_all_interval_boundaries() {
        let mut frontier = Frontier::new(5);
        for (slot, score) in [2.0, 5.0, 1.0, 4.0, 0.0].into_iter().enumerate() {
            assert_eq!(frontier.insert(candidate(slot, &[]), score), slot);
        }
        assert_invariants(&frontier);
        assert_eq!(frontier.total_cached_score(), 12.0);

        for (mass, expected_slot) in [
            (0.0, 0),
            (1.0, 0),
            (1.999, 0),
            (2.0, 1),
            (3.0, 1),
            (6.999, 1),
            (7.0, 2),
            (7.5, 2),
            (7.999, 2),
            (8.0, 3),
            (10.0, 3),
            (11.999, 3),
        ] {
            assert_eq!(frontier.propose(mass), expected_slot);
        }
    }

    #[test]
    fn score_updates_removal_and_replacement_repair_all_aggregates() {
        let mut frontier = Frontier::new(3);
        let first = candidate(0, &[]);
        let second = candidate(1, &[]);
        let third = candidate(2, &[]);
        assert_eq!(frontier.insert(Arc::clone(&first), 5.0), 0);
        assert_eq!(frontier.insert(Arc::clone(&second), 2.0), 1);
        assert_eq!(frontier.insert(Arc::clone(&third), 7.0), 2);
        assert_invariants(&frontier);
        assert_eq!(frontier.minimum_cached_score(), Some((2.0, 1)));

        frontier.update_score(1, 1.0);
        assert_invariants(&frontier);
        assert_eq!(frontier.minimum_cached_score(), Some((1.0, 1)));

        frontier.update_score(2, 1.0);
        assert_invariants(&frontier);
        assert_eq!(
            frontier.minimum_cached_score(),
            Some((1.0, 1)),
            "equal minima use the lowest stable slot",
        );

        frontier.update_score(2, 0.5);
        assert_invariants(&frontier);
        assert_eq!(frontier.minimum_cached_score(), Some((0.5, 2)));

        frontier.update_score(0, 0.0);
        assert_invariants(&frontier);
        assert_eq!(frontier.minimum_cached_score(), Some((0.0, 0)));

        assert!(Arc::ptr_eq(&frontier.remove(0), &first));
        assert_invariants(&frontier);
        assert_eq!(frontier.minimum_cached_score(), Some((0.5, 2)));

        let reinserted = candidate(41, &[]);
        assert_eq!(frontier.insert(Arc::clone(&reinserted), 6.0), 0);
        assert_invariants(&frontier);
        assert!(Arc::ptr_eq(&frontier.active_slot(0).candidate, &reinserted,));

        let replacement = candidate(42, &[]);
        assert!(Arc::ptr_eq(
            &frontier.replace(1, Arc::clone(&replacement), 4.0),
            &second,
        ));
        assert_invariants(&frontier);
        assert!(Arc::ptr_eq(
            &frontier.active_slot(1).candidate,
            &replacement,
        ));
        assert_eq!(frontier.minimum_cached_score(), Some((0.5, 2)));
    }

    #[test]
    fn admission_inserts_while_not_full() {
        let mut locations = [Location::new(2.0)];
        let admitted = candidate(7, &[0]);
        let mut frontier = Frontier::new(2);
        record_trace(&admitted.visited_locations, &mut locations);

        match admit_after_recording_trace(&mut frontier, Arc::clone(&admitted), &locations) {
            Admission::Inserted { slot } => assert_eq!(slot, 0),
            Admission::Replaced { .. } | Admission::Rejected { .. } => {
                panic!("candidate should use a free slot")
            }
        }
        assert_invariants(&frontier);
        assert!(Arc::ptr_eq(&frontier.active_slot(0).candidate, &admitted));
    }

    #[test]
    fn admission_uses_post_execution_visit_count() {
        let mut locations = [Location::new(2.0)];
        let admitted = candidate(7, &[0]);
        let mut frontier = Frontier::new(1);

        record_trace(&admitted.visited_locations, &mut locations);
        assert!(matches!(
            admit_after_recording_trace(&mut frontier, admitted, &locations),
            Admission::Inserted { slot: 0 },
        ));
        assert_eq!(
            frontier.active_slot(0).cached_score.value(),
            1.0,
            "the execution's visit must affect its admitted score",
        );
        assert_invariants(&frontier);
    }

    #[test]
    fn admission_uses_certain_cached_minimum_replacement() {
        let mut locations = [Location::new(1.0), Location::new(2.0), Location::new(3.0)];
        let victim = candidate(0, &[0]);
        let survivor = candidate(1, &[1]);
        let admitted = candidate(2, &[2]);
        let mut frontier = Frontier::new(2);
        frontier.insert(Arc::clone(&victim), exact_score(&victim, &locations));
        frontier.insert(Arc::clone(&survivor), exact_score(&survivor, &locations));
        record_trace(&admitted.visited_locations, &mut locations);

        match admit_after_recording_trace(&mut frontier, Arc::clone(&admitted), &locations) {
            Admission::Replaced { slot, evicted } => {
                assert_eq!(slot, 0);
                assert!(Arc::ptr_eq(&evicted, &victim));
            }
            Admission::Inserted { .. } | Admission::Rejected { .. } => {
                panic!("strictly larger score should replace cached minimum")
            }
        }
        assert_invariants(&frontier);
        assert!(Arc::ptr_eq(&frontier.active_slot(0).candidate, &admitted));
    }

    #[test]
    fn admission_recomputes_victim_then_replaces() {
        let mut locations = [Location::new(4.0), Location::new(5.0), Location::new(4.0)];
        let victim = candidate(0, &[0]);
        let survivor = candidate(1, &[1]);
        let admitted = candidate(2, &[2]);
        let mut frontier = Frontier::new(2);
        frontier.insert(Arc::clone(&victim), exact_score(&victim, &locations));
        frontier.insert(Arc::clone(&survivor), exact_score(&survivor, &locations));
        for _ in 0..3 {
            record_trace(&[0], &mut locations);
        }
        record_trace(&admitted.visited_locations, &mut locations);

        match admit_after_recording_trace(&mut frontier, Arc::clone(&admitted), &locations) {
            Admission::Replaced { slot, evicted } => {
                assert_eq!(slot, 0);
                assert!(Arc::ptr_eq(&evicted, &victim));
            }
            Admission::Inserted { .. } | Admission::Rejected { .. } => {
                panic!("candidate should beat the corrected victim score")
            }
        }
        assert_invariants(&frontier);
        assert_eq!(frontier.active_slot(0).cached_score.value(), 2.0);
    }

    #[test]
    fn admission_recomputes_victim_then_rejects() {
        let mut locations = [Location::new(4.0), Location::new(5.0), Location::new(1.5)];
        let victim = candidate(0, &[0]);
        let survivor = candidate(1, &[1]);
        let rejected = candidate(2, &[2]);
        let mut frontier = Frontier::new(2);
        frontier.insert(Arc::clone(&victim), exact_score(&victim, &locations));
        frontier.insert(Arc::clone(&survivor), exact_score(&survivor, &locations));
        record_trace(&[0], &mut locations);
        record_trace(&rejected.visited_locations, &mut locations);

        match admit_after_recording_trace(&mut frontier, Arc::clone(&rejected), &locations) {
            Admission::Rejected { candidate } => {
                assert!(Arc::ptr_eq(&candidate, &rejected));
            }
            Admission::Inserted { .. } | Admission::Replaced { .. } => {
                panic!("candidate should lose to the corrected victim score")
            }
        }
        assert_invariants(&frontier);
        assert_eq!(frontier.active_slot(0).cached_score.value(), 2.0);
        assert!(Arc::ptr_eq(&frontier.active_slot(0).candidate, &victim));
    }

    #[test]
    fn admission_rejects_equal_scores() {
        let mut locations = [Location::new(2.0), Location::new(3.0), Location::new(4.0)];
        let victim = candidate(0, &[0]);
        let survivor = candidate(1, &[1]);
        let rejected = candidate(2, &[2]);
        let mut frontier = Frontier::new(2);
        frontier.insert(Arc::clone(&victim), exact_score(&victim, &locations));
        frontier.insert(Arc::clone(&survivor), exact_score(&survivor, &locations));
        record_trace(&rejected.visited_locations, &mut locations);

        assert!(matches!(
            admit_after_recording_trace(&mut frontier, Arc::clone(&rejected), &locations),
            Admission::Rejected { .. },
        ));
        assert_invariants(&frontier);
        assert!(Arc::ptr_eq(&frontier.active_slot(0).candidate, &victim));
    }

    #[test]
    fn lazy_sampling_accepts_immediately() {
        let locations = [Location::new(2.0)];
        let selected = candidate(0, &[0]);
        let mut frontier = Frontier::new(1);
        frontier.insert(Arc::clone(&selected), 2.0);
        let mut draws = VecDeque::from([(
            2.0,
            SamplingDraw {
                proposal_mass: 0.0,
                acceptance_value: 0.999,
            },
        )]);
        let mut draw = |total| {
            let (expected_total, draw) = draws.pop_front().unwrap();
            assert_eq!(total, expected_total);
            draw
        };

        let sampled = sample_with_draws(&mut frontier, &locations, &mut draw).unwrap();
        assert!(Arc::ptr_eq(&sampled, &selected));
        assert!(draws.is_empty());
        assert_invariants(&frontier);
    }

    #[test]
    fn lazy_sampling_rejects_repairs_and_proposes_again() {
        let mut locations = [Location::new(4.0), Location::new(2.0)];
        let stale = candidate(0, &[0]);
        let selected = candidate(1, &[1]);
        let mut frontier = Frontier::new(2);
        frontier.insert(Arc::clone(&stale), 4.0);
        frontier.insert(Arc::clone(&selected), 2.0);
        for _ in 0..3 {
            record_trace(&[0], &mut locations);
        }
        let mut draws = VecDeque::from([
            (
                6.0,
                SamplingDraw {
                    proposal_mass: 0.0,
                    acceptance_value: 0.5,
                },
            ),
            (
                3.0,
                SamplingDraw {
                    proposal_mass: 1.0,
                    acceptance_value: 0.5,
                },
            ),
        ]);
        let mut draw = |total| {
            let (expected_total, draw) = draws.pop_front().unwrap();
            assert_eq!(total, expected_total);
            draw
        };

        let sampled = sample_with_draws(&mut frontier, &locations, &mut draw).unwrap();
        assert!(Arc::ptr_eq(&sampled, &selected));
        assert_eq!(frontier.active_slot(0).cached_score.value(), 1.0);
        assert!(draws.is_empty());
        assert_invariants(&frontier);
    }

    #[test]
    fn lazy_sampling_repairs_a_candidate_that_decays_to_zero() {
        let mut locations = [location(f64::MIN_POSITIVE, u64::MAX), Location::new(1.0)];
        let decayed = candidate(0, &[0]);
        let selected = candidate(1, &[1]);
        let mut frontier = Frontier::new(2);
        locations[0].visits = 0;
        frontier.insert(Arc::clone(&decayed), exact_score(&decayed, &locations));
        frontier.insert(Arc::clone(&selected), exact_score(&selected, &locations));
        locations[0].visits = u64::MAX;
        let mut draws = VecDeque::from([
            SamplingDraw {
                proposal_mass: 0.0,
                acceptance_value: 0.0,
            },
            SamplingDraw {
                proposal_mass: 0.0,
                acceptance_value: 0.0,
            },
        ]);
        let mut draw = |_total| draws.pop_front().unwrap();

        let sampled = sample_with_draws(&mut frontier, &locations, &mut draw).unwrap();
        assert!(Arc::ptr_eq(&sampled, &selected));
        assert_eq!(frontier.active_slot(0).cached_score.value(), 0.0);
        assert!(draws.is_empty());
        assert_invariants(&frontier);
    }

    #[test]
    fn lazy_sampling_returns_none_after_all_candidates_decay_to_zero() {
        let mut locations = [Location::new(f64::MIN_POSITIVE); 2];
        let first = candidate(0, &[0]);
        let second = candidate(1, &[1]);
        let mut frontier = Frontier::new(2);
        frontier.insert(Arc::clone(&first), exact_score(&first, &locations));
        frontier.insert(Arc::clone(&second), exact_score(&second, &locations));
        locations[0].visits = u64::MAX;
        locations[1].visits = u64::MAX;
        let mut draws = VecDeque::from([
            SamplingDraw {
                proposal_mass: 0.0,
                acceptance_value: 0.0,
            },
            SamplingDraw {
                proposal_mass: 0.0,
                acceptance_value: 0.0,
            },
        ]);
        let mut draw = |_total| draws.pop_front().unwrap();

        assert!(
            sample_with_draws(&mut frontier, &locations, &mut draw).is_none(),
            "all corrected scores are zero",
        );
        assert_eq!(frontier.total_cached_score(), 0.0);
        assert!(draws.is_empty());
        assert_invariants(&frontier);
    }

    #[test]
    fn scoring_uses_multipliers_visits_and_unique_execution_counts() {
        let mut locations = [location(2.0, 0), location(6.0, 2), location(0.0, 99)];
        let scored = candidate((), &[0, 1, 2]);
        assert_eq!(exact_score(&scored, &locations), 4.0);

        record_trace(&[0, 1], &mut locations);
        assert_eq!(locations[0].visits, 1);
        assert_eq!(locations[1].visits, 3);
        let decreased = exact_score(&scored, &locations);
        assert_eq!(decreased, 2.5);
        assert!(decreased < 4.0);

        record_trace(&[0, 1], &mut locations);
        assert_eq!(locations[0].visits, 2);
        assert_eq!(locations[1].visits, 4);
        assert!(exact_score(&scored, &locations) < decreased);
    }

    #[test]
    fn invalid_floating_point_state_is_rejected() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            assert!(catch_unwind(|| Location::new(invalid)).is_err());
            assert!(
                catch_unwind(|| {
                    let mut frontier = Frontier::new(1);
                    frontier.insert(candidate((), &[]), invalid);
                })
                .is_err(),
            );
        }
    }

    #[test]
    fn invalid_execution_trace_does_not_partially_increment_locations() {
        let mut locations = [Location::new(1.0), Location::new(1.0)];
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                record_trace(&[0, 2], &mut locations);
            }))
            .is_err(),
        );
        assert_eq!(locations[0].visits, 0);
        assert_eq!(locations[1].visits, 0);
    }

    #[test]
    #[should_panic(expected = "coverage traces must be sorted and duplicate-free")]
    fn duplicate_candidate_trace_is_rejected() {
        let _candidate = Candidate::new((), vec![1, 1].into_boxed_slice());
    }

    #[test]
    #[should_panic(expected = "coverage traces must be sorted and duplicate-free")]
    fn unsorted_candidate_trace_is_rejected() {
        let _candidate = Candidate::new((), vec![2, 1].into_boxed_slice());
    }

    #[test]
    #[should_panic(expected = "coverage traces must be sorted and duplicate-free")]
    fn duplicate_execution_trace_is_rejected() {
        record_trace(&[1, 1], &mut [Location::new(1.0), Location::new(1.0)]);
    }

    #[test]
    #[should_panic(expected = "coverage traces must be sorted and duplicate-free")]
    fn unsorted_execution_trace_is_rejected() {
        record_trace(&[1, 0], &mut [Location::new(1.0), Location::new(1.0)]);
    }

    #[test]
    #[should_panic(expected = "coverage location ID is out of range")]
    fn out_of_range_candidate_location_is_rejected() {
        let scored = candidate((), &[1]);
        let _score = exact_score(&scored, &[Location::new(1.0)]);
    }

    #[test]
    fn visit_overflow_is_rejected_before_any_location_is_incremented() {
        let mut locations = [Location::new(1.0), location(1.0, u64::MAX)];
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                record_trace(&[0, 1], &mut locations);
            }))
            .is_err(),
        );
        assert_eq!(locations[0].visits, 0);
        assert_eq!(locations[1].visits, u64::MAX);
    }

    #[test]
    #[should_panic(expected = "coverage frontier capacity must be nonzero")]
    fn zero_frontier_capacity_is_rejected() {
        let _frontier = Frontier::<()>::new(0);
    }

    #[test]
    #[should_panic(expected = "sampling mass must be below total cached score")]
    fn proposal_rejects_mass_at_total() {
        let mut frontier = Frontier::new(1);
        frontier.insert(candidate((), &[]), 1.0);
        let _slot = frontier.propose(1.0);
    }

    #[test]
    #[should_panic(expected = "sampling mass must be nonnegative")]
    fn proposal_rejects_negative_mass() {
        let mut frontier = Frontier::new(1);
        frontier.insert(candidate((), &[]), 1.0);
        let _slot = frontier.propose(-1.0);
    }

    #[test]
    #[should_panic(expected = "exact coverage score increased during recomputation")]
    fn lazy_sampling_rejects_score_increases() {
        let locations = [Location::new(2.0)];
        let inconsistent = candidate((), &[0]);
        let mut frontier = Frontier::new(1);
        frontier.insert(inconsistent, 1.0);
        let mut draw = |_total| SamplingDraw {
            proposal_mass: 0.0,
            acceptance_value: 0.0,
        };
        let _sampled = sample_with_draws(&mut frontier, &locations, &mut draw);
    }
}
