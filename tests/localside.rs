use core::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use hibana_runtime::{__private::block_on, scope};
#[path = "support/driver.rs"]
mod driver;
use driver::TestDriver;

type Outcome<T = ()> = Result<T, &'static str>;

async fn yield_once() {
    let mut yielded = false;
    core::future::poll_fn(|cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await;
}

fn run<F: Future>(future: F) -> F::Output {
    let mut waits = 0;
    block_on(
        future,
        TestDriver(Waker::noop(), || {
            waits += 1;
            assert!(waits < 1000, "test localsides stalled");
        }),
    )
}

#[test]
fn declarations_and_the_rest_of_the_async_body_make_independent_progress() {
    #[scope]
    async fn program() -> Outcome<u32> {
        let ready = Cell::new(false);
        let child_done = Cell::new(false);
        async {
            async {
                core::future::poll_fn(|_| {
                    if ready.get() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                })
                .await;
                child_done.set(true);
                Ok::<(), &'static str>(())
            };
            // This must run while the earlier child is Pending.
            yield_once().await;
            ready.set(true);
            Ok(())
        }
        .await?;
        assert!(child_done.get());
        Ok(42)
    }
    assert_eq!(run(program()), Ok(42));
}

#[test]
fn received_owned_values_can_be_borrowed_concurrently_and_returned_after_the_scope() {
    struct View(u32);
    #[scope]
    async fn worker(mut left: View, mut right: View) -> Outcome<(View, View)> {
        async {
            async {
                let borrowed = &mut left.0;
                yield_once().await;
                *borrowed += 1;
                Ok::<(), &'static str>(())
            };
            async {
                let borrowed = &mut right.0;
                yield_once().await;
                *borrowed += 2;
                Ok::<(), &'static str>(())
            };
            Ok(())
        }
        .await?;
        Ok((left, right))
    }
    let (left, right) = run(worker(View(7), View(40))).unwrap();
    assert_eq!((left.0, right.0), (8, 42));
}

#[test]
fn a_later_declaration_starts_when_its_statement_is_reached() {
    #[scope]
    async fn program() -> Outcome {
        let before_second = Cell::new(false);
        let second_ran = Cell::new(false);
        async {
            async {
                while !second_ran.get() {
                    yield_once().await;
                }
                Ok::<(), &'static str>(())
            };
            yield_once().await;
            before_second.set(true);
            async {
                assert!(before_second.get());
                second_ran.set(true);
                Ok::<(), &'static str>(())
            };
            Ok(())
        }
        .await?;
        Ok(())
    }
    run(program()).unwrap();
}

#[test]
fn early_return_finishes_children_before_releasing_borrows() {
    #[scope]
    async fn program(value: &Cell<u32>) -> Outcome<u32> {
        async {
            yield_once().await;
            value.set(19);
            Ok::<(), &'static str>(())
        };
        return Ok(7);
    }
    let value = Cell::new(0);
    assert_eq!(run(program(&value)), Ok(7));
    assert_eq!(value.get(), 19);
}

#[test]
fn nested_scopes_preserve_returns_and_typed_block_values() {
    #[scope]
    async fn program(early: bool) -> Outcome<u32> {
        async {
            yield_once().await;
            Ok::<(), &'static str>(())
        };
        let answer = async {
            async {
                yield_once().await;
                Ok::<(), &'static str>(())
            };
            if early {
                return Ok(31);
            }
            Ok(42)
        }
        .await?;
        Ok(answer)
    }
    assert_eq!(run(program(true)), Ok(31));
    assert_eq!(run(program(false)), Ok(42));
}

#[test]
fn loops_labels_break_values_and_continue_survive_the_async_boundary() {
    #[scope]
    async fn program() -> Outcome<u32> {
        let count = Cell::new(0);
        async {
            yield_once().await;
        };
        let answer = 'outer: loop {
            count.set(count.get() + 1);
            if count.get() < 3 {
                continue 'outer;
            }
            let mut inner_count = 39;
            let inner = loop {
                inner_count += 1;
                if inner_count == 40 {
                    break inner_count;
                }
                yield_once().await;
            };
            break 'outer inner + count.get();
        };
        let block = 'block: {
            if count.get() == 3 {
                break 'block 7;
            }
            0
        };
        for _ in 0..2 {
            if count.get() == 3 {
                count.set(4);
                continue;
            }
            break;
        }
        Ok(answer + block + count.get())
    }
    assert_eq!(run(program()), Ok(54));
}

struct Never<'a>(&'a Cell<u32>);
impl Future for Never<'_> {
    type Output = Outcome;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Outcome> {
        Poll::Pending
    }
}
impl Drop for Never<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn child_error_cancels_pending_siblings_and_the_parent() {
    #[scope]
    async fn program(drops: &Cell<u32>) -> Outcome {
        async { Never(drops).await };
        async {
            yield_once().await;
            Err::<(), _>("child")
        };
        core::future::pending::<()>().await;
        Ok(())
    }
    let drops = Cell::new(0);
    assert_eq!(run(program(&drops)), Err("child"));
    assert_eq!(drops.get(), 1);
}

#[test]
fn parent_question_mark_cancels_children() {
    #[scope]
    async fn program(drops: &Cell<u32>) -> Outcome {
        async { Never(drops).await };
        Err::<(), _>("parent")?;
        Ok(())
    }
    let drops = Cell::new(0);
    assert_eq!(run(program(&drops)), Err("parent"));
    assert_eq!(drops.get(), 1);
}

#[test]
fn returned_and_tail_errors_cancel_children() {
    #[scope]
    async fn returned(drops: &Cell<u32>) -> Outcome {
        async { Never(drops).await };
        return Err("returned");
    }
    #[scope]
    async fn tail(drops: &Cell<u32>) -> Outcome {
        async { Never(drops).await };
        Err("tail")
    }
    let drops = Cell::new(0);
    assert_eq!(run(returned(&drops)), Err("returned"));
    assert_eq!(run(tail(&drops)), Err("tail"));
    assert_eq!(drops.get(), 2);
}

#[test]
fn async_values_closures_and_explicit_awaits_keep_ordinary_rust_semantics() {
    #[scope]
    async fn program() -> Outcome<u32> {
        let not_started = Cell::new(0);
        let future = async {
            not_started.set(3);
            7
        };
        let closure = || {
            if not_started.get() == 3 {
                return 8;
            }
            0
        };
        async {
            yield_once().await;
            Ok::<(), &'static str>(())
        };
        assert_eq!(not_started.get(), 0);
        let value = future.await;
        Ok(value + closure())
    }
    assert_eq!(run(program()), Ok(15));
}

#[test]
fn entry_attribute_drives_a_non_unit_result_and_scoped_borrows() {
    #[hibana_runtime::main(TestDriver(Waker::noop(), || {}))]
    async fn entry(value: &mut u32) -> Outcome<u32> {
        async {
            async {
                yield_once().await;
                *value = 42;
                Ok::<(), &'static str>(())
            };
            Ok(())
        }
        .await?;
        Ok(*value)
    }
    let mut value = 0;
    assert_eq!(entry(&mut value), Ok(42));
    assert_eq!(value, 42);
}

#[test]
fn statement_bodies_infer_success_and_keep_conditional_compilation() {
    #[scope]
    async fn program(value: &mut u32) -> Outcome {
        async {
            #[cfg(any())]
            async {
                this_is_not_a_function();
            };
            #[cfg(target_pointer_width = "64")]
            async {
                yield_once().await;
                *value = 42;
            };
            async {};
            async {
                async {
                    yield_once().await;
                };
                async {
                    yield_once().await;
                };
            };
            Ok(())
        }
        .await?;
        Ok(())
    }
    let mut value = 0;
    assert_eq!(run(program(&mut value)), Ok(()));
    assert_eq!(value, 42);
}

#[test]
fn infallible_errors_are_inferred_for_tail_values_and_early_returns() {
    #[scope]
    async fn tail() -> Result<(), core::convert::Infallible> {
        async {
            yield_once().await;
        };
        Ok(())
    }
    #[scope]
    async fn early() -> Result<u32, core::convert::Infallible> {
        async {
            yield_once().await;
        };
        return Ok(42);
    }
    assert_eq!(run(tail()), Ok(()));
    assert_eq!(run(early()), Ok(42));
}

#[test]
fn named_localsides_are_constructed_at_declaration_and_driven_without_wrappers() {
    fn task<'a>(
        constructed: &'a Cell<u32>,
        finished: &'a Cell<u32>,
    ) -> impl Future<Output = Outcome> + 'a {
        constructed.set(constructed.get() + 1);
        async move {
            // Both constructors must run before either future is polled.
            assert_eq!(constructed.get(), 2);
            let bytes = [21_u8];
            let borrowed = &bytes[0];
            yield_once().await;
            finished.set(finished.get() + u32::from(*borrowed));
            Ok(())
        }
    }
    #[scope]
    async fn program() -> Outcome<u32> {
        let constructed = Cell::new(0);
        let finished = Cell::new(0);
        async {
            #[concurrent]
            task(&constructed, &finished);
            #[concurrent]
            task(&constructed, &finished);
            Ok(())
        }
        .await?;
        Ok(finished.get())
    }
    assert_eq!(run(program()), Ok(42));
}

#[test]
fn named_localsides_support_methods_paths_macros_cfg_and_cancellation() {
    struct Work<'a>(&'a Cell<u32>);
    impl Work<'_> {
        async fn execute(&self) -> Outcome {
            yield_once().await;
            self.0.set(self.0.get() + 1);
            Ok(())
        }
    }
    macro_rules! future {
        () => {
            async {
                yield_once().await;
                Ok::<(), &'static str>(())
            }
        };
    }
    #[scope]
    async fn program(finished: &Cell<u32>, drops: &Cell<u32>) -> Outcome {
        let work = Work(finished);
        async {
            #[concurrent]
            #[cfg(any())]
            not_defined();
            #[concurrent]
            work.execute();
            #[concurrent]
            future!();
            Ok(())
        }
        .await?;
        let pending = Never(drops);
        #[concurrent]
        pending;
        Err("cancel")
    }
    let finished = Cell::new(0);
    let drops = Cell::new(0);
    assert_eq!(run(program(&finished, &drops)), Err("cancel"));
    assert_eq!(finished.get(), 1);
    assert_eq!(drops.get(), 1);
}

#[test]
fn constructor_error_drops_previously_declared_unpolled_futures() {
    fn fail() -> Outcome<Never<'static>> {
        Err("construct")
    }
    #[scope]
    async fn program(drops: &Cell<u32>) -> Outcome {
        #[concurrent]
        Never(drops);
        #[concurrent]
        fail()?;
        Ok(())
    }
    let drops = Cell::new(0);
    assert_eq!(run(program(&drops)), Err("construct"));
    assert_eq!(drops.get(), 1);
}

#[test]
fn ordinary_blocks_do_not_poll_or_finish_an_unawaited_async_body() {
    #[scope]
    async fn program() -> Outcome {
        let completed = Cell::new(false);
        let future = async {
            async {
                yield_once().await;
                completed.set(true);
            };
            Ok(42)
        };
        {
            let reference = &future;
            core::hint::black_box(reference);
        }
        assert!(!completed.get());
        assert_eq!(future.await?, 42);
        assert!(completed.get());
        Ok(())
    }
    run(program()).unwrap();
}

#[test]
fn conditional_and_repeated_async_bodies_keep_explicit_await_boundaries() {
    #[scope]
    async fn program(enabled: bool, completed: &Cell<u32>) -> Outcome {
        if enabled {
            for _ in 0..3 {
                async {
                    async {
                        yield_once().await;
                        completed.set(completed.get() + 1);
                    };
                    async {
                        yield_once().await;
                        completed.set(completed.get() + 1);
                    };
                    Ok(())
                }
                .await?;
            }
        }
        Ok(())
    }
    let completed = Cell::new(0);
    run(program(false, &completed)).unwrap();
    assert_eq!(completed.get(), 0);
    run(program(true, &completed)).unwrap();
    assert_eq!(completed.get(), 6);
}

#[test]
fn unit_control_flow_needs_no_boilerplate_success_values() {
    #[scope]
    async fn program(completed: &Cell<u32>) -> Outcome {
        async {
            for _ in 0..2 {
                yield_once().await;
                completed.set(completed.get() + 1);
            }
        };
        async {
            let mut remaining = 2;
            while remaining != 0 {
                yield_once().await;
                completed.set(completed.get() + 1);
                remaining -= 1;
            }
        };
        async {
            if core::hint::black_box(true) {
                yield_once().await;
                completed.set(completed.get() + 1);
            }
        };
        Ok(())
    }
    let completed = Cell::new(0);
    run(program(&completed)).unwrap();
    assert_eq!(completed.get(), 5);
}

#[test]
fn unused_and_wildcard_arguments_remain_owned_by_an_unpolled_future() {
    #[scope]
    async fn unused(_resource: Never) -> Outcome {
        Ok(())
    }
    #[scope]
    async fn wildcard(_: Never) -> Outcome {
        Ok(())
    }
    let drops = Cell::new(0);
    let future = unused(Never(&drops));
    assert_eq!(drops.get(), 0);
    drop(future);
    assert_eq!(drops.get(), 1);
    let future = wildcard(Never(&drops));
    assert_eq!(drops.get(), 1);
    drop(future);
    assert_eq!(drops.get(), 2);
}

#[test]
fn argument_destructuring_and_receiver_ownership_match_async_functions() {
    async fn native((resource, _): (Never<'_>, Never<'_>), drops: &Cell<u32>) -> Outcome {
        assert_eq!(drops.get(), 0);
        yield_once().await;
        drop(resource);
        Ok(())
    }
    #[scope]
    async fn generated((resource, _): (Never, Never), drops: &Cell<u32>) -> Outcome {
        assert_eq!(drops.get(), 0);
        yield_once().await;
        drop(resource);
        Ok(())
    }
    let native_drops = Cell::new(0);
    let generated_drops = Cell::new(0);
    let native = native((Never(&native_drops), Never(&native_drops)), &native_drops);
    let generated = generated(
        (Never(&generated_drops), Never(&generated_drops)),
        &generated_drops,
    );
    assert_eq!(native_drops.get(), generated_drops.get());
    run(native).unwrap();
    run(generated).unwrap();
    assert_eq!(native_drops.get(), 2);
    assert_eq!(generated_drops.get(), 2);

    struct Task<'a> {
        _resource: Never<'a>,
    }
    impl Task<'_> {
        #[scope]
        async fn execute(self) -> Outcome {
            Ok(())
        }
    }
    let drops = Cell::new(0);
    let future = Task {
        _resource: Never(&drops),
    }
    .execute();
    assert_eq!(drops.get(), 0);
    drop(future);
    assert_eq!(drops.get(), 1);
}

#[test]
fn a_result_binding_does_not_wait_for_an_adjacent_scoped_child() {
    #[scope]
    async fn image(stopped: &Cell<bool>) -> Outcome<u32> {
        let reply_used = Cell::new(false);
        async {
            while !reply_used.get() {
                yield_once().await;
            }
            stopped.set(true);
        };
        #[concurrent]
        let reply = async {
            yield_once().await;
            Ok(42)
        };
        reply_used.set(true);
        Ok(reply)
    }
    let stopped = Cell::new(false);
    assert_eq!(run(image(&stopped)), Ok(42));
    assert!(stopped.get());
}

#[test]
fn separate_result_bindings_wait_at_their_own_source_positions() {
    fn request(order: &Cell<u8>, expected: u8) -> impl Future<Output = Outcome<u8>> {
        assert_eq!(order.get(), expected, "request constructed too early");
        async move {
            yield_once().await;
            order.set(expected + 1);
            Ok(expected)
        }
    }
    #[scope]
    async fn image(order: &Cell<u8>) -> Outcome<(u8, u8)> {
        #[concurrent]
        let first = request(order, 0);
        #[concurrent]
        let second = request(order, 1);
        Ok((first, second))
    }
    let order = Cell::new(0);
    assert_eq!(run(image(&order)), Ok((0, 1)));
    assert_eq!(order.get(), 2);
}

#[test]
fn a_result_is_available_to_an_immediately_following_scoped_child() {
    #[scope]
    async fn image(value: &Cell<u32>) -> Outcome {
        #[concurrent]
        let reply = async {
            yield_once().await;
            Ok(42)
        };
        async {
            yield_once().await;
            value.set(reply);
        };
        Ok(())
    }
    let value = Cell::new(0);
    run(image(&value)).unwrap();
    assert_eq!(value.get(), 42);
}

#[test]
fn a_result_group_error_cancels_an_earlier_scoped_child() {
    #[scope]
    async fn image(drops: &Cell<u32>, constructed: &Cell<bool>) -> Outcome {
        async { Never(drops).await };
        #[concurrent]
        let (_first, _second) = (async { Ok(7_u8) }, async {
            yield_once().await;
            Err::<(), _>("request failed")
        });
        #[concurrent]
        let _later = {
            constructed.set(true);
            async { Ok(()) }
        };
        Ok(())
    }
    let (drops, constructed) = (Cell::new(0), Cell::new(false));
    assert_eq!(run(image(&drops, &constructed)), Err("request failed"));
    assert_eq!(drops.get(), 1);
    assert!(!constructed.get());
}

#[test]
fn an_explicit_result_group_releases_borrows_before_the_continuation() {
    #[scope]
    async fn program(left: &mut u32, right: &mut u32) -> Outcome<(u32, u32)> {
        let result = async {
            #[concurrent]
            let (a, b) = (
                async {
                    yield_once().await;
                    *left += 1;
                    Ok::<_, &'static str>(*left)
                },
                async {
                    yield_once().await;
                    *right += 2;
                    Ok::<_, &'static str>(*right)
                },
            );
            Ok((a, b))
        }
        .await?;
        *left += 10;
        *right += 20;
        Ok(result)
    }
    let mut left = 0;
    let mut right = 0;
    assert_eq!(run(program(&mut left, &mut right)), Ok((1, 2)));
    assert_eq!((left, right), (11, 22));
}

#[test]
fn a_result_group_drives_different_futures_and_releases_their_borrows() {
    async fn first(value: &mut u32, started: &Cell<u8>) -> Outcome<u32> {
        started.set(started.get() | 1);
        while started.get() != 3 {
            yield_once().await;
        }
        *value += 1;
        Ok(*value)
    }
    #[scope]
    async fn image(left: &mut u32, right: &mut bool) -> Outcome<(u32, bool)> {
        let started = Cell::new(0);
        #[concurrent]
        let (a, b) = (first(left, &started), async {
            started.set(started.get() | 2);
            while started.get() != 3 {
                yield_once().await;
            }
            *right = true;
            Ok(*right)
        });
        *left += 10;
        *right = false;
        Ok((a, b))
    }
    let (mut left, mut right) = (7, false);
    assert_eq!(run(image(&mut left, &mut right)), Ok((8, true)));
    assert_eq!((left, right), (18, false));
}

#[test]
fn result_groups_keep_constructor_order_cfg_and_nested_child_completion() {
    fn task<'a>(order: &'a Cell<u8>, index: u8) -> impl Future<Output = Outcome<u8>> + 'a {
        assert_eq!(order.get(), index);
        order.set(index + 1);
        async move {
            assert_eq!(order.get(), 2);
            yield_once().await;
            Ok(index)
        }
    }
    #[scope]
    async fn image(order: &Cell<u8>, child: &Cell<bool>) -> Outcome {
        #[concurrent]
        #[cfg(any())]
        let (_absent_a, _absent_b) = (undefined(), undefined());
        #[concurrent]
        let (a, b, c) = (task(order, 0), task(order, 1), async {
            async {
                yield_once().await;
                child.set(true);
            };
            Ok(7_u16)
        });
        assert_eq!((a, b, c), (0, 1, 7));
        assert!(child.get());
        Ok(())
    }
    run(image(&Cell::new(0), &Cell::new(false))).unwrap();
}

#[test]
fn grouped_results_can_borrow_the_callers_inputs_after_the_scope_returns() {
    #[scope]
    async fn image<'a>(
        left: &'a mut u32,
        right: &'a mut bool,
    ) -> Outcome<(&'a mut u32, &'a mut bool)> {
        #[concurrent]
        let (left, right) = (
            async {
                yield_once().await;
                Ok(left)
            },
            async {
                yield_once().await;
                Ok(right)
            },
        );
        Ok((left, right))
    }
    let (mut left, mut right) = (7, false);
    let (a, b) = run(image(&mut left, &mut right)).unwrap();
    *a = 42;
    *b = true;
    assert_eq!((left, right), (42, true));
}

#[test]
fn a_failed_result_group_cancels_its_siblings_and_drops_completed_results() {
    #[derive(Debug)]
    struct Value<'a>(&'a Cell<u32>);
    impl Drop for Value<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[scope]
    async fn image(pending: &Cell<u32>, completed: &Cell<u32>) -> Outcome {
        #[concurrent]
        let (_pending, _completed, _failed) =
            (Never(pending), async { Ok(Value(completed)) }, async {
                yield_once().await;
                Err::<(), _>("failed")
            });
        panic!("a failed result group must not run its continuation")
    }
    let (pending, completed) = (Cell::new(0), Cell::new(0));
    assert_eq!(run(image(&pending, &completed)), Err("failed"));
    assert_eq!((pending.get(), completed.get()), (1, 1));
}

#[test]
fn a_group_constructor_error_drops_earlier_unpolled_inputs_once() {
    fn fail() -> Outcome<Never<'static>> {
        Err("construct")
    }
    #[scope]
    async fn image(drops: &Cell<u32>) -> Outcome {
        #[concurrent]
        let (_pending, _failed) = (Never(drops), fail()?);
        panic!("a failed constructor must not run its continuation")
    }
    let drops = Cell::new(0);
    assert_eq!(run(image(&drops)), Err("construct"));
    assert_eq!(drops.get(), 1);
}

#[test]
fn nested_result_bindings_infer_an_error_with_multiple_conversions() {
    #[derive(Debug, PartialEq)]
    struct Error;
    impl From<u8> for Error {
        fn from(_: u8) -> Self {
            Self
        }
    }
    impl From<u16> for Error {
        fn from(_: u16) -> Self {
            Self
        }
    }

    #[scope]
    async fn program() -> Result<u32, Error> {
        let (a, b) = async {
            #[concurrent]
            let (a, b) = (
                async {
                    hibana_runtime::yield_now().await;
                    Ok(7u32)
                },
                async {
                    hibana_runtime::yield_now().await;
                    Ok(8u32)
                },
            );
            Ok((a, b))
        }
        .await?;
        Ok(a + b)
    }
    assert_eq!(run(program()), Ok(15));
}

#[test]
fn named_localsides_convert_their_errors_and_cancel_a_pending_sibling() {
    #[derive(Debug, PartialEq)]
    struct Error(u16);
    impl From<u8> for Error {
        fn from(value: u8) -> Self {
            Self(value.into())
        }
    }
    impl From<u16> for Error {
        fn from(value: u16) -> Self {
            Self(value)
        }
    }
    async fn fail() -> Result<(), u8> {
        yield_once().await;
        Err(7)
    }
    async fn waiting(drops: &Cell<u32>) -> Result<(), u16> {
        let _owned = Never(drops);
        core::future::pending().await
    }
    #[scope]
    async fn program(drops: &Cell<u32>, continued: &Cell<bool>) -> Result<(), Error> {
        #[concurrent]
        fail();
        #[concurrent]
        waiting(drops);
        core::future::pending::<()>().await;
        continued.set(true);
        Ok(())
    }
    let drops = Cell::new(0);
    let continued = Cell::new(false);
    assert_eq!(run(program(&drops, &continued)), Err(Error(7)));
    assert_eq!(drops.get(), 1);
    assert!(!continued.get());
}

#[test]
fn a_bound_result_can_borrow_a_callers_view_after_the_scope_returns() {
    struct View(u32);
    impl View {
        async fn borrow_value(&mut self) -> Outcome<&mut u32> {
            yield_once().await;
            Ok(&mut self.0)
        }
    }
    #[scope]
    async fn borrow(view: &mut View) -> Outcome<&mut u32> {
        #[concurrent]
        let value = view.borrow_value();
        Ok(value)
    }
    #[scope]
    async fn borrow_inline(view: &mut View) -> Outcome<&mut u32> {
        #[concurrent]
        let value = async { view.borrow_value().await };
        Ok(value)
    }
    let mut view = View(7);
    let value = run(borrow(&mut view)).unwrap();
    *value += 1;
    assert_eq!(view.0, 8);
    let value = run(borrow_inline(&mut view)).unwrap();
    *value += 1;
    assert_eq!(view.0, 9);
}

#[test]
fn failed_result_binding_cancels_its_pending_sibling_without_running_the_continuation() {
    struct OnDrop<'a>(&'a Cell<u32>);
    impl Drop for OnDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[scope]
    async fn program(dropped: &Cell<u32>) -> Outcome {
        let probe = OnDrop(dropped);
        #[concurrent]
        let (a, b) = (
            async move {
                let _probe = probe;
                core::future::pending::<()>().await;
                Ok::<_, &'static str>(())
            },
            async {
                yield_once().await;
                Err::<(), _>("failed")
            },
        );
        let _ = (a, b);
        panic!("failed localsides must not run the continuation")
    }
    let dropped = Cell::new(0);
    assert_eq!(run(program(&dropped)), Err("failed"));
    assert_eq!(dropped.get(), 1);
}

#[test]
fn declared_array_localsides_overlap_and_keep_borrowed_inputs() {
    #[scope]
    async fn image(values: [&Cell<u8>; 3], arrived: &Cell<u8>) -> Outcome {
        #[concurrent]
        for (bit, value) in [(1, values[0]), (2, values[1]), (4, values[2])] {
            arrived.set(arrived.get() | bit);
            while arrived.get() != 7 {
                yield_once().await;
            }
            value.set(bit);
        }
        Ok(())
    }
    let values = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let arrived = Cell::new(0);
    run(image(values.each_ref(), &arrived)).unwrap();
    assert_eq!(values.map(Cell::into_inner), [1, 2, 4]);
}

#[test]
fn an_empty_declaration_array_finishes_without_a_poll_slot() {
    #[scope]
    async fn image(values: [u8; 0]) -> Outcome {
        #[concurrent]
        for value in values {
            assert_eq!(value, 0);
        }
        Ok(())
    }
    run(image([])).unwrap();
}

#[test]
fn declaration_array_input_awaits_its_nested_localsides_before_returning() {
    #[scope]
    async fn image(done: &Cell<bool>, seen: &Cell<u8>) -> Outcome {
        #[concurrent]
        for value in (async {
            async {
                yield_once().await;
                done.set(true);
            };
            Ok::<_, &'static str>([7, 11])
        })
        .await?
        {
            assert!(done.get());
            seen.set(seen.get() + value);
        }
        assert!(done.get());
        Ok(())
    }
    let done = Cell::new(false);
    let seen = Cell::new(0);
    assert_eq!(run(image(&done, &seen)), Ok(()));
    assert_eq!(seen.get(), 18);
}

#[test]
fn declaration_array_input_child_error_prevents_participant_creation() {
    #[scope]
    async fn image(created: &Cell<bool>) -> Outcome {
        #[concurrent]
        for _ in (async {
            async {
                yield_once().await;
                Err::<(), _>("input child failed")
            };
            Ok::<_, &'static str>([0])
        })
        .await?
        {
            created.set(true);
        }
        panic!("failed array input must not run its continuation")
    }
    let created = Cell::new(false);
    assert_eq!(run(image(&created)), Err("input child failed"));
    assert!(!created.get());
}

#[test]
fn declaration_array_input_can_return_borrows_updated_by_its_children() {
    #[scope]
    async fn image(values: &[Cell<u8>; 2]) -> Outcome {
        #[concurrent]
        for value in (async {
            async {
                yield_once().await;
                values[0].set(7);
                values[1].set(11);
            };
            Ok::<_, &'static str>(values.each_ref())
        })
        .await?
        {
            value.set(value.get() + 1);
        }
        Ok(())
    }
    let values = [Cell::new(0), Cell::new(0)];
    assert_eq!(run(image(&values)), Ok(()));
    assert_eq!(values.map(Cell::into_inner), [8, 12]);
}

#[test]
fn an_array_child_failure_drops_even_unpolled_owned_inputs_once() {
    struct Input<'a> {
        index: usize,
        drops: &'a [Cell<u8>; 3],
    }
    impl Drop for Input<'_> {
        fn drop(&mut self) {
            self.drops[self.index].set(self.drops[self.index].get() + 1);
        }
    }
    #[scope]
    async fn image(values: [Input; 3]) -> Outcome {
        #[concurrent]
        for value in values {
            let owned = value;
            if owned.index == 0 {
                return Err("child failed");
            }
            core::future::pending::<()>().await;
        }
        Ok(())
    }
    let drops = core::array::from_fn(|_| Cell::new(0));
    let values = core::array::from_fn(|index| Input {
        index,
        drops: &drops,
    });
    assert_eq!(run(image(values)), Err("child failed"));
    assert_eq!(drops.map(Cell::into_inner), [1, 1, 1]);
}
