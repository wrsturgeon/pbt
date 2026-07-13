# Untranslated QuixBugs Programs

The initial benchmark admits only inputs composed from ordinary Rust standard-library types and
excludes user-error implementations that may not terminate.

## `bitcount`

The user-error loop does not terminate for ordinary inputs such as `1`.

## `breadth_first_search`

The user-error implementation loops forever when the goal is unreachable and also requires
identity-bearing graph nodes.

## `depth_first_search`

The program requires identity-bearing graph nodes.

## `detect_cycle`

The program requires an identity-bearing cyclic linked-list structure.

## `find_in_sorted`

The user-error recursion can repeat the same interval forever.

## `find_first_in_sorted`

The user-error loop can repeat the same interval forever when the target is smaller than the
smallest element.

## `flatten`

Faithful input requires a recursive heterogeneous list type.

## `gcd`

The user-error recursion can repeat the same argument pair forever.

## `max_sublist_sum`

The defect requires signed integers, for which `pbt` does not currently provide its standard
registration.

## `mergesort`

The user-error recursion does not terminate on singleton lists.

## `minimum_spanning_tree`

The program is graph-based and its source representation requires a map keyed by undirected edges.

## `reverse_linked_list`

The program requires an identity-bearing mutable linked-list structure.

## `rpn_eval`

Faithful input requires a heterogeneous token type containing both numbers and operators.

## `shortest_path_length`

The program is graph-based and its source representation requires an edge map.

## `shortest_path_lengths`

The program is graph-based and its source representation requires an edge map.

## `shortest_paths`

The program is graph-based, requires an edge map, and admits negative weights.

## `shunting_yard`

Faithful input requires a heterogeneous token type containing both numbers and operators.

## `sqrt`

The user-error iteration may not converge, and `pbt` does not currently provide standard
registration for floating-point inputs.

## `topological_ordering`

The program requires identity-bearing graph nodes with incoming and outgoing adjacency sets.

## `wrap`

The user-error loop can stop consuming input after splitting at a space, so ordinary text inputs
can make it repeat forever.
