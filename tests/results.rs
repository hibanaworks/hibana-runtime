use core::{cell::Cell, future::Future, task::Waker};
use hibana_runtime::{Driver, scope};

struct Ready(usize);

impl Driver for Ready {
    fn waker(&self) -> Waker {
        Waker::noop().clone()
    }

    fn wait(&mut self) {
        self.0 += 1;
        assert!(self.0 < 100, "a scoped result stopped making progress");
    }
}

fn run<T>(future: impl Future<Output = T>) -> T {
    hibana_runtime::__private::block_on(future, Ready(0))
}

#[test]
fn a_variable_tuple_drives_both_inputs_before_returning_values() {
    #[scope]
    async fn program(started: &Cell<u8>) -> Result<(u32, bool), ()> {
        let inputs = (
            async {
                started.set(started.get() | 1);
                while started.get() != 3 {
                    hibana_runtime::yield_now().await;
                }
                Ok::<_, ()>(7)
            },
            async {
                started.set(started.get() | 2);
                while started.get() != 3 {
                    hibana_runtime::yield_now().await;
                }
                Ok::<_, ()>(true)
            },
        );
        #[concurrent]
        let (first, second) = inputs;
        assert_eq!(started.get(), 3);
        Ok((first, second))
    }
    assert_eq!(run(program(&Cell::new(0))), Ok((7, true)));
}

#[test]
fn one_future_returning_a_tuple_is_one_declared_child() {
    #[scope]
    async fn program() -> Result<(u32, bool), ()> {
        #[concurrent]
        let pair = async {
            hibana_runtime::yield_now().await;
            Ok((7, true))
        };
        let (number, enabled) = pair;
        Ok((number, enabled))
    }
    assert_eq!(run(program()), Ok((7, true)));
}

#[test]
fn arrays_and_nested_arrays_preserve_their_actual_shape() {
    #[scope]
    async fn program() -> Result<([u32; 3], [[u32; 2]; 2], [u32; 0]), ()> {
        let input = [1, 2, 3].map(|value| async move { Ok::<_, ()>(value * 2) });
        #[concurrent]
        let values = input;
        #[concurrent]
        let nested =
            [[1, 2], [3, 4]].map(|row| row.map(|value| core::future::ready(Ok::<_, ()>(value))));
        #[concurrent]
        let empty = [] as [core::future::Ready<Result<u32, ()>>; 0];
        Ok((values, nested, empty))
    }
    assert_eq!(run(program()), Ok(([2, 4, 6], [[1, 2], [3, 4]], [])));
}

#[test]
fn result_values_can_keep_mutable_borrows_of_the_callers_data() {
    #[scope]
    async fn program<'a>(
        left: &'a mut u32,
        right: &'a mut bool,
    ) -> Result<(&'a mut u32, &'a mut bool), ()> {
        #[concurrent]
        let (first, second) = (async { Ok(left) }, async { Ok(right) });
        Ok((first, second))
    }
    let (mut left, mut right) = (7, false);
    let (first, second) = run(program(&mut left, &mut right)).unwrap();
    *first = 8;
    *second = true;
    assert_eq!((left, right), (8, true));
}

#[test]
fn a_non_clone_child_error_stops_the_declaration_before_its_continuation() {
    #[derive(Debug, PartialEq)]
    struct Error(u32);

    #[scope]
    async fn program(after: &Cell<bool>) -> Result<(), Error> {
        #[concurrent]
        let _failed = async {
            hibana_runtime::yield_now().await;
            Err::<u32, _>(Error(7))
        };
        after.set(true);
        Ok(())
    }
    let after = Cell::new(false);
    assert_eq!(run(program(&after)), Err(Error(7)));
    assert!(!after.get());
}

#[test]
fn inline_flows_use_their_values_without_a_result_channel() {
    #[scope]
    async fn program(published: &Cell<u32>, consumed: &Cell<bool>) -> Result<(), ()> {
        async {
            let value = async { Ok::<_, ()>(7) }.await?;
            published.set(value);
            while !consumed.get() {
                hibana_runtime::yield_now().await;
            }
        };
        async {
            while published.get() == 0 {
                hibana_runtime::yield_now().await;
            }
            assert_eq!(published.get(), 7);
            consumed.set(true);
        };
        Ok(())
    }
    let (published, consumed) = (Cell::new(0), Cell::new(false));
    assert_eq!(run(program(&published, &consumed)), Ok(()));
    assert!(consumed.get());
}

#[test]
fn function_if_and_match_inputs_are_evaluated_once_at_the_declaration() {
    fn inputs(
        calls: &Cell<u32>,
        number: u32,
    ) -> (
        impl Future<Output = Result<u32, ()>>,
        impl Future<Output = Result<bool, ()>>,
    ) {
        calls.set(calls.get() + 1);
        (
            core::future::ready(Ok(number)),
            core::future::ready(Ok(true)),
        )
    }

    #[scope]
    async fn program(calls: &Cell<u32>, select: bool) -> Result<(), ()> {
        #[concurrent]
        let (a, b) = if select {
            inputs(calls, 7)
        } else {
            inputs(calls, 8)
        };
        #[concurrent]
        let (c, d) = match select {
            true => inputs(calls, 9),
            false => inputs(calls, 10),
        };
        assert_eq!(
            (a, b, c, d),
            if select {
                (7, true, 9, true)
            } else {
                (8, true, 10, true)
            }
        );
        Ok(())
    }
    for select in [true, false] {
        let calls = Cell::new(0);
        run(program(&calls, select)).unwrap();
        assert_eq!(calls.get(), 2);
    }
}

#[test]
fn array_patterns_bind_the_completed_values() {
    #[scope]
    async fn program() -> Result<(u32, u32), ()> {
        let input = [7, 8].map(|value| core::future::ready(Ok::<_, ()>(value)));
        #[concurrent]
        let [first, second] = input;
        Ok((first, second))
    }
    assert_eq!(run(program()), Ok((7, 8)));
}

#[test]
fn tuple_declarations_do_not_use_a_fixed_trait_arity_table() {
    #[scope]
    async fn program() -> Result<[u8; 17], ()> {
        #[concurrent]
        let (a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q) = (
            async { Ok(0) },
            async { Ok(1) },
            async { Ok(2) },
            async { Ok(3) },
            async { Ok(4) },
            async { Ok(5) },
            async { Ok(6) },
            async { Ok(7) },
            async { Ok(8) },
            async { Ok(9) },
            async { Ok(10) },
            async { Ok(11) },
            async { Ok(12) },
            async { Ok(13) },
            async { Ok(14) },
            async { Ok(15) },
            async { Ok(16) },
        );
        Ok([a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q])
    }
    assert_eq!(
        run(program()),
        Ok(core::array::from_fn(|index| index as u8))
    );
}

#[test]
fn joined_values_have_ordinary_rust_drop_semantics() {
    struct Value<'a>(&'a Cell<u32>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[scope]
    async fn program(finished: &Cell<bool>, drops: &Cell<u32>) -> Result<(), ()> {
        #[concurrent]
        let value = async {
            hibana_runtime::yield_now().await;
            finished.set(true);
            Ok(Value(drops))
        };
        assert!(finished.get());
        assert_eq!(drops.get(), 0);
        drop(value);
        assert_eq!(drops.get(), 1);
        Ok(())
    }
    let finished = Cell::new(false);
    let drops = Cell::new(0);
    run(program(&finished, &drops)).unwrap();
    assert_eq!(drops.get(), 1);
}

#[test]
fn an_input_error_drops_a_siblings_completed_value_once() {
    struct Value<'a>(&'a Cell<u32>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[scope]
    async fn program(drops: &Cell<u32>) -> Result<(), ()> {
        #[concurrent]
        let (_value, _failed) = (async { Ok(Value(drops)) }, async {
            hibana_runtime::yield_now().await;
            Err::<(), _>(())
        });
        panic!("failed inputs must not reach their continuation")
    }
    let drops = Cell::new(0);
    assert_eq!(run(program(&drops)), Err(()));
    assert_eq!(drops.get(), 1);
}

#[test]
fn successful_result_values_are_not_structurally_pinned() {
    struct Value<'a> {
        value: &'a mut u32,
        _pin: core::marker::PhantomPinned,
    }
    #[scope]
    async fn program<'a>(value: &'a mut u32) -> Result<Value<'a>, ()> {
        #[concurrent]
        let result = async {
            let scratch = [7_u8; 64];
            let borrowed = &scratch;
            hibana_runtime::yield_now().await;
            *value += u32::from(borrowed[3]);
            Ok(Value {
                value,
                _pin: core::marker::PhantomPinned,
            })
        };
        Ok(result)
    }
    let mut number = 1;
    let value = run(program(&mut number)).unwrap();
    *value.value += 1;
    assert_eq!(number, 9);
}
