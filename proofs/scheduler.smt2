; Independent scheduler arithmetic / slot-presence model for src/lib.rs.
; This is not a Rust refinement or a protocol, Waker, or pin-safety proof.
; Each block first checks satisfiable premises, then refutes a bad outcome.
; Expected result sequence: (sat unsat) repeated eight times.
; Constant arrays use Z3's array extensions.
(set-logic ALL)
(declare-const n Int)
(declare-const first Int)
(declare-const offset Int)
(declare-const task Int)
(declare-const a Int)
(declare-const b Int)
(declare-const limit Int)
(define-fun in-range ((i Int)) Bool (and (<= 0 i) (< i n)))
(define-fun next ((i Int)) Int (ite (= i (- n 1)) 0 (+ i 1)))
(define-fun sweep-index ((k Int)) Int
  (ite (< (+ first k) n) (+ first k) (- (+ first k) n)))
(define-fun position ((i Int)) Int
  (ite (< i first) (+ (- n first) i) (- i first)))

(push)
(assert (in-range first))
(assert (in-range offset))
(check-sat)
; The two range traversals have exactly the rotating sweep's order.
(define-fun split-index ((k Int)) Int
  (ite (< k (- n first)) (+ first k) (- k (- n first))))
(assert (not (= (split-index offset) (sweep-index offset))))
(check-sat)
(pop)

(push)
(assert (> n 0))
(assert (in-range first))
(check-sat)
; The next first/index remains in bounds and wraps the final slot to zero.
(assert (or (not (in-range (next first)))
  (and (= first (- n 1)) (not (= (next first) 0)))))
(check-sat)
(pop)

(push)
(assert (in-range first))
(assert (<= 0 offset))
(assert (< (+ offset 1) n))
(check-sat)
; The closed-form sweep follows the exact branch used by the Rust loop.
(assert (not (= (next (sweep-index offset))
  (sweep-index (+ offset 1)))))
(check-sat)
(pop)

(push)
(assert (in-range first))
(assert (in-range task))
(check-sat)
; Every registered slot has an offset in one non-failing sweep.
(assert (or (not (in-range (position task)))
  (not (= (sweep-index (position task)) task))))
(check-sat)
(pop)

(push)
(assert (in-range a))
(assert (in-range b))
(check-sat)
; No slot is visited twice within a sweep.
(assert (= (sweep-index a) (sweep-index b)))
(assert (not (= a b)))
(check-sat)
(pop)

(declare-const live (Array Int Bool))
(declare-const removed Int)
(declare-const queried Int)
(push)
(check-sat)
; Replacing one Some registration by None cannot re-enable another slot.
(assert (select (store live removed false) queried))
(assert (not (select live queried)))
(check-sat)
(pop)

(push)
(assert (in-range queried))
(check-sat)
; The error path empties the entire registration array before returning.
(assert (select ((as const (Array Int Bool)) false) queried))
(check-sat)
(pop)

(push)
(assert (in-range first))
(assert (<= n limit))
(check-sat)
; Incrementing any valid index cannot exceed the target maximum. Rust uses
; the increment only for a non-final index, wrapping the final index to zero.
(assert (> (+ first 1) limit))
(check-sat)
(pop)
