import Std

/-!
An independent mathematical model of the scheduler loop in `src/lib.rs`.

`next` is the Rust wrap branch; `sweepIndex` is its closed form for a single
N-slot sweep. A live slot models `Some(Registration)`, and clearing models
replacement by `None`. These theorems concern slot scheduling only. They do
not prove Rust refinement, Waker behavior, pin safety, or Hibana protocol
progress. `tests/runtime.rs` connects the modeled cases to executable code.
The coverage theorem assumes no task returns an error during the sweep;
Rust intentionally stops and cancels the remaining registrations on error.
-/
namespace HibanaRuntime.Scheduler

def next (n index : Nat) : Nat :=
  if index = n - 1 then 0 else index + 1

def sweepIndex (n first offset : Nat) : Nat :=
  if first + offset < n then first + offset else first + offset - n

def position (n first task : Nat) : Nat :=
  if task < first then n - first + task else task - first

-- Runtime traverses [first, n) followed by [0, first), using inline storage.
def splitIndex (n first offset : Nat) : Nat :=
  if offset < n - first then first + offset else offset - (n - first)

theorem split_ranges_match_sweep (n first offset : Nat)
    (start : first < n) :
    splitIndex n first offset = sweepIndex n first offset := by
  unfold splitIndex sweepIndex
  split <;> split <;> omega

theorem next_in_bounds (n index : Nat) (within : index < n) :
    next n index < n := by
  unfold next
  split <;> omega

theorem last_wraps (n : Nat) : next n (n - 1) = 0 := by
  simp [next]

theorem sweep_starts_at_first (n first : Nat) (within : first < n) :
    sweepIndex n first 0 = first := by
  simp [sweepIndex, within]

theorem sweep_index_in_bounds (n first offset : Nat)
    (start : first < n) (within : offset < n) :
    sweepIndex n first offset < n := by
  unfold sweepIndex
  split <;> omega

theorem sweep_step_matches_wrap (n first offset : Nat)
    (start : first < n) (within : offset + 1 < n) :
    next n (sweepIndex n first offset) =
      sweepIndex n first (offset + 1) := by
  by_cases before : first + offset < n
  · simp only [sweepIndex, before, ite_true, next]
    split <;> split <;> omega
  · simp only [sweepIndex, before, ite_false, next]
    split <;> split <;> omega

theorem position_in_sweep (n first task : Nat)
    (start : first < n) (registered : task < n) :
    position n first task < n := by
  unfold position
  split <;> omega

theorem sweep_visits_slot (n first task : Nat)
    (start : first < n) (registered : task < n) :
    sweepIndex n first (position n first task) = task := by
  by_cases earlier : task < first
  · simp only [position, earlier, ite_true, sweepIndex]
    split <;> omega
  · simp only [position, earlier, ite_false, sweepIndex]
    split <;> omega

theorem sweep_visits_at_most_once (n first a b : Nat)
    (aWithin : a < n) (bWithin : b < n)
    (same : sweepIndex n first a = sweepIndex n first b) : a = b := by
  unfold sweepIndex at same
  split at same <;> split at same <;> omega

def sweep (n first : Nat) : List Nat :=
  (List.range n).map (sweepIndex n first)

theorem every_registered_slot_is_visited (n first task : Nat)
    (start : first < n) (registered : task < n) : task ∈ sweep n first := by
  apply List.mem_map.mpr
  exact ⟨position n first task,
    List.mem_range.mpr (position_in_sweep n first task start registered),
    sweep_visits_slot n first task start registered⟩

def liveSweep (n first : Nat) (live : Nat → Bool) : List Nat :=
  (sweep n first).filter live

theorem every_live_slot_is_polled_without_failfast (n first task : Nat)
    (live : Nat → Bool) (start : first < n) (registered : task < n)
    (isLive : live task = true) : task ∈ liveSweep n first live := by
  apply List.mem_filter.mpr
  exact ⟨every_registered_slot_is_visited n first task start registered, isLive⟩

def clear (live : Nat → Bool) (slot : Nat) : Nat → Bool :=
  fun i => if i = slot then false else live i

theorem clear_removes_slot (live : Nat → Bool) (slot : Nat) :
    clear live slot slot = false := by
  simp [clear]

theorem clear_never_reenables (live : Nat → Bool) (slot i : Nat)
    (stillLive : clear live slot i = true) : live i = true := by
  by_cases same : i = slot
  · simp [clear, same] at stillLive
  · simpa [clear, same] using stillLive

def clearMany : List Nat → (Nat → Bool) → (Nat → Bool)
  | [], live => live
  | slot :: rest, live => clearMany rest (clear live slot)

theorem clearMany_never_reenables (slots : List Nat) (live : Nat → Bool) (i : Nat)
    (stillLive : clearMany slots live i = true) : live i = true := by
  induction slots generalizing live with
  | nil => exact stillLive
  | cons slot rest ih =>
    exact clear_never_reenables live slot i (ih (clear live slot) stillLive)

-- Every valid index can be incremented without exceeding usize's maximum,
-- including the final index. Rust uses the increment only before that final
-- index and otherwise wraps to zero.
theorem increment_stays_within_usize (n index limit : Nat)
    (within : index < n) (capacity : n ≤ limit) : index + 1 ≤ limit := by
  omega

-- Inferred storage uses binary carries so its Rust type does not grow as a
-- linear chain. This independent model checks that carrying preserves the
-- localsides' registration order; it is not a Rust layout or pin-safety proof.
inductive Chunk where
  | leaf (id : Nat)
  | pair (left right : Chunk)

def Chunk.values : Chunk → List Nat
  | .leaf id => [id]
  | .pair left right => left.values ++ right.values

inductive Storage where
  | empty
  | vacant (tail : Storage)
  | full (head : Chunk) (tail : Storage)

def Storage.values : Storage → List Nat
  | .empty => []
  | .vacant tail => tail.values
  | .full head tail => tail.values ++ head.values

def insertChunk : Storage → Chunk → Storage
  | .empty, chunk => .full chunk .empty
  | .vacant tail, chunk => .full chunk tail
  | .full head tail, chunk => .vacant (insertChunk tail (.pair head chunk))

theorem storage_insert_preserves_order (storage : Storage) (chunk : Chunk) :
    (insertChunk storage chunk).values = storage.values ++ chunk.values := by
  induction storage generalizing chunk with
  | empty => simp [insertChunk, Storage.values]
  | vacant tail _ => simp [insertChunk, Storage.values]
  | full head tail ih =>
    simp [insertChunk, Storage.values, Chunk.values, ih, List.append_assoc]

#print axioms next_in_bounds
#print axioms split_ranges_match_sweep
#print axioms last_wraps
#print axioms sweep_starts_at_first
#print axioms sweep_index_in_bounds
#print axioms sweep_step_matches_wrap
#print axioms position_in_sweep
#print axioms sweep_visits_slot
#print axioms sweep_visits_at_most_once
#print axioms every_registered_slot_is_visited
#print axioms every_live_slot_is_polled_without_failfast
#print axioms clear_removes_slot
#print axioms clear_never_reenables
#print axioms clearMany_never_reenables
#print axioms increment_stays_within_usize
#print axioms storage_insert_preserves_order

end HibanaRuntime.Scheduler
