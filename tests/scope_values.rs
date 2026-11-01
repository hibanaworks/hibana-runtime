use core::{cell::Cell, future::Future, task::Waker};
use hibana_runtime::{Driver, scope};

struct Ready(usize);
impl Driver for Ready {
    fn waker(&self) -> Waker {
        Waker::noop().clone()
    }
    fn wait(&mut self) {
        self.0 += 1;
        assert!(self.0 < 100, "scope stopped making progress");
    }
}
fn run<T>(future: impl Future<Output = T>) -> T {
    hibana_runtime::__private::block_on(future, Ready(0))
}

#[test]
fn unmarked_children_return_data_in_declaration_order_after_both_make_progress() {
    #[scope]
    async fn program(started: &Cell<u8>) -> Result<(u32, bool), ()> {
        async {
            started.set(started.get() | 1);
            while started.get() != 3 {
                hibana_runtime::yield_now().await;
            }
            Ok(7)
        };
        async {
            started.set(started.get() | 2);
            while started.get() != 3 {
                hibana_runtime::yield_now().await;
            }
            Ok(true)
        };
    }
    assert_eq!(run(program(&Cell::new(0))), Ok((7, true)));
}

#[test]
fn values_stay_flat_when_parent_effects_separate_child_declarations() {
    #[scope]
    async fn program(effect: &Cell<u8>) -> Result<(u32, bool, char), ()> {
        async {
            hibana_runtime::yield_now().await;
            Ok(7)
        };
        effect.set(1);
        async {
            hibana_runtime::yield_now().await;
            Ok(true)
        };
        effect.set(2);
        async { Ok('x') };
    }
    let effect = Cell::new(0);
    assert_eq!(run(program(&effect)), Ok((7, true, 'x')));
    assert_eq!(effect.get(), 2);
}

#[test]
fn early_parent_success_overrides_collected_values_and_still_waits_for_children() {
    #[scope]
    async fn program(done: &Cell<bool>, early: bool) -> Result<(u32, bool), ()> {
        async {
            hibana_runtime::yield_now().await;
            done.set(true);
            Ok(7)
        };
        if early {
            return Ok((9, false));
        }
        async { Ok(true) };
    }
    for early in [true, false] {
        let done = Cell::new(false);
        assert_eq!(
            run(program(&done, early)),
            Ok(if early { (9, false) } else { (7, true) })
        );
        assert!(done.get());
    }
}

#[test]
fn a_parent_return_before_any_declaration_does_not_construct_children() {
    #[scope]
    async fn program(entered: &Cell<bool>, early: bool) -> Result<u32, ()> {
        if early {
            return Ok(9);
        }
        async {
            entered.set(true);
            Ok(7)
        };
    }
    for early in [true, false] {
        let entered = Cell::new(false);
        assert_eq!(run(program(&entered, early)), Ok(if early { 9 } else { 7 }));
        assert_eq!(entered.get(), !early);
    }
}

#[test]
fn explicit_parent_tail_keeps_its_own_value() {
    #[scope]
    async fn program(done: &Cell<bool>) -> Result<char, ()> {
        async {
            hibana_runtime::yield_now().await;
            done.set(true);
            Ok(7)
        };
        Ok('x')
    }
    let done = Cell::new(false);
    assert_eq!(run(program(&done)), Ok('x'));
    assert!(done.get());
}

#[test]
fn an_async_scope_is_a_composable_future_and_returns_borrowed_values() {
    #[scope]
    async fn program<'a>(
        first: &'a mut u32,
        second: &'a mut bool,
    ) -> Result<(&'a mut u32, &'a mut bool), ()> {
        let values = async {
            async {
                hibana_runtime::yield_now().await;
                Ok(first)
            };
            async { Ok(second) };
        };
        values.await
    }
    let (mut number, mut enabled) = (7, false);
    let (first, second) = run(program(&mut number, &mut enabled)).unwrap();
    *first += 1;
    *second = true;
    assert_eq!((number, enabled), (8, true));
}

#[test]
fn a_child_error_cancels_pending_siblings_and_parent_suffix() {
    struct Held<'a>(&'a Cell<bool>);
    impl Drop for Held<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    #[derive(Debug, PartialEq)]
    struct Error(u32);
    #[scope]
    async fn program(held: Held<'_>, after: &Cell<bool>) -> Result<(u32, bool), Error> {
        async move {
            let _held = held;
            core::future::pending::<()>().await;
            Ok(7)
        };
        after.set(true);
        async {
            hibana_runtime::yield_now().await;
            Err::<bool, _>(Error(3))
        };
    }
    let (dropped, after) = (Cell::new(false), Cell::new(false));
    assert_eq!(run(program(Held(&dropped), &after)), Err(Error(3)));
    assert!(dropped.get());
    assert!(after.get());
}

#[test]
fn fixed_arrays_return_arrays_without_a_declaration_attribute() {
    #[scope]
    async fn program() -> Result<[u32; 3], ()> {
        for number in [1, 2, 3] {
            async move {
                hibana_runtime::yield_now().await;
                Ok(number * 2)
            };
        }
    }
    assert_eq!(run(program()), Ok([2, 4, 6]));
}

#[test]
fn nested_fixed_arrays_keep_their_shape_and_empty_arrays_finish() {
    #[scope]
    async fn program() -> Result<([[u32; 2]; 2], [u32; 0]), ()> {
        async {
            for row in [[1, 2], [3, 4]] {
                async move {
                    for number in row {
                        async move {
                            hibana_runtime::yield_now().await;
                            Ok(number)
                        };
                    }
                };
            }
        };
        async {
            for number in [] as [u32; 0] {
                async move { Ok(number) };
            }
        };
    }
    assert_eq!(run(program()), Ok(([[1, 2], [3, 4]], [])));
}

#[test]
fn an_array_input_awaits_nested_children_before_constructing_participants() {
    #[scope]
    async fn program(done: &Cell<bool>) -> Result<[u32; 1], ()> {
        for number in (async {
            async {
                hibana_runtime::yield_now().await;
                done.set(true);
            };
            Ok::<_, ()>([7])
        })
        .await?
        {
            async {
                assert!(done.get());
                Ok(number)
            };
        }
    }
    assert_eq!(run(program(&Cell::new(false))), Ok([7]));
}

#[test]
fn ordinary_loops_stay_sequential_and_stored_futures_stay_lazy() {
    #[scope]
    async fn program(touched: &Cell<bool>) -> Result<u32, ()> {
        let future = async {
            touched.set(true);
            Ok::<_, ()>(99)
        };
        let mut number = 0;
        for next in [1, 2, 3] {
            number += next;
            hibana_runtime::yield_now().await;
        }
        drop(future);
        async { Ok(number) };
    }
    let touched = Cell::new(false);
    assert_eq!(run(program(&touched)), Ok(6));
    assert!(!touched.get());
}

#[test]
fn an_immediate_scope_error_is_a_value_until_its_caller_propagates_it() {
    #[scope]
    async fn program(after: &Cell<bool>) -> Result<u32, ()> {
        let result = async {
            async {
                hibana_runtime::yield_now().await;
                Ok(7)
            };
            Err::<(), _>(())?;
            async { Ok(true) };
        }
        .await;
        assert_eq!(result, Err::<(u32, bool), _>(()));
        after.set(true);
        Ok(9)
    }
    let after = Cell::new(false);
    assert_eq!(run(program(&after)), Ok(9));
    assert!(after.get());
}

#[test]
fn an_immediate_scope_return_does_not_return_from_the_enclosing_participant() {
    #[scope]
    async fn program(after: &Cell<bool>) -> Result<u32, ()> {
        let value = async {
            async {
                hibana_runtime::yield_now().await;
                Ok(7)
            };
            if !after.get() {
                return Ok((9, false));
            }
            async { Ok(true) };
        }
        .await?;
        assert_eq!(value, (9, false));
        after.set(true);
        Ok(11)
    }
    let after = Cell::new(false);
    assert_eq!(run(program(&after)), Ok(11));
    assert!(after.get());
}

#[test]
fn a_direct_await_converts_a_child_error_through_the_enclosing_error() {
    #[derive(Debug, PartialEq)]
    struct Error(u8);
    impl From<u8> for Error {
        fn from(value: u8) -> Self {
            Error(value)
        }
    }
    #[scope]
    async fn program() -> Result<u32, Error> {
        async { core::future::ready(Err::<u32, u8>(7)).await };
    }
    assert_eq!(run(program()), Err(Error(7)));
}

#[test]
fn forwarded_future_storage_never_moves_after_its_first_poll() {
    struct Anchored<'a> {
        address: Cell<*const Self>,
        dropped: &'a Cell<bool>,
        _pinned: core::marker::PhantomPinned,
    }
    impl Future for Anchored<'_> {
        type Output = Result<u32, ()>;
        fn poll(
            self: core::pin::Pin<&mut Self>,
            cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<Self::Output> {
            let value = self.as_ref().get_ref();
            if value.address.get().is_null() {
                value.address.set(value);
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            } else {
                assert_eq!(value.address.get(), value as *const _);
                core::task::Poll::Ready(Ok(7))
            }
        }
    }
    impl Drop for Anchored<'_> {
        fn drop(&mut self) {
            assert_eq!(self.address.get(), self as *const _);
            assert!(!self.dropped.replace(true));
        }
    }
    #[scope]
    async fn program(dropped: &Cell<bool>) -> Result<u32, ()> {
        let input = Anchored {
            address: Cell::new(core::ptr::null()),
            dropped,
            _pinned: core::marker::PhantomPinned,
        };
        async { input.await };
    }
    let dropped = Cell::new(false);
    assert_eq!(run(program(&dropped)), Ok(7));
    assert!(dropped.get());
}

#[test]
fn completed_values_are_destroyed_once_when_a_later_parent_step_fails() {
    struct Value<'a>(&'a Cell<u32>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[scope]
    async fn program(drops: &Cell<u32>) -> Result<(), ()> {
        let _values = async {
            async { Ok(Value(drops)) };
            hibana_runtime::yield_now().await;
            Err::<(), _>(())?;
            async { Ok(true) };
        }
        .await?;
        Ok(())
    }
    let drops = Cell::new(0);
    assert_eq!(run(program(&drops)), Err(()));
    assert_eq!(drops.get(), 1);
}
