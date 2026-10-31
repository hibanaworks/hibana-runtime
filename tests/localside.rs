use core::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use hibana_runtime::{__private::block_on, localside};
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
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
    async fn returned(drops: &Cell<u32>) -> Outcome {
        async { Never(drops).await };
        return Err("returned");
    }
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
    async fn tail() -> Result<(), core::convert::Infallible> {
        async {
            yield_once().await;
        };
        Ok(())
    }
    #[localside]
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
    #[localside]
    async fn program() -> Outcome<u32> {
        let constructed = Cell::new(0);
        let finished = Cell::new(0);
        async {
            #[localside]
            task(&constructed, &finished);
            #[hibana_runtime::localside]
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
    #[localside]
    async fn program(finished: &Cell<u32>, drops: &Cell<u32>) -> Outcome {
        let work = Work(finished);
        async {
            #[localside]
            #[cfg(any())]
            not_defined();
            #[localside]
            work.execute();
            #[localside]
            future!();
            Ok(())
        }
        .await?;
        let pending = Never(drops);
        #[localside]
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
    #[localside]
    async fn program(drops: &Cell<u32>) -> Outcome {
        #[localside]
        Never(drops);
        #[localside]
        fail()?;
        Ok(())
    }
    let drops = Cell::new(0);
    assert_eq!(run(program(&drops)), Err("construct"));
    assert_eq!(drops.get(), 1);
}

#[test]
fn ordinary_blocks_do_not_poll_or_finish_an_unawaited_async_body() {
    #[localside]
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
    #[localside]
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
    #[localside]
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
    #[localside]
    async fn unused(_resource: Never) -> Outcome {
        Ok(())
    }
    #[localside]
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
    #[localside]
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
        #[localside]
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
fn result_bindings_run_concurrently_and_release_borrows_before_the_continuation() {
    #[localside]
    async fn program(left: &mut u32, right: &mut u32) -> Outcome<(u32, u32)> {
        let result = async {
            #[localside]
            let a = async {
                yield_once().await;
                *left += 1;
                Ok::<_, &'static str>(*left)
            };
            #[localside]
            let b = async {
                yield_once().await;
                *right += 2;
                Ok::<_, &'static str>(*right)
            };
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
fn failed_result_binding_cancels_its_pending_sibling_without_running_the_continuation() {
    struct OnDrop<'a>(&'a Cell<u32>);
    impl Drop for OnDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[localside]
    async fn program(dropped: &Cell<u32>) -> Outcome {
        let probe = OnDrop(dropped);
        #[localside]
        let a = async move {
            let _probe = probe;
            core::future::pending::<()>().await;
            Ok::<_, &'static str>(())
        };
        #[localside]
        let b = async {
            yield_once().await;
            Err::<(), _>("failed")
        };
        let _ = (a, b);
        panic!("failed localsides must not run the continuation")
    }
    let dropped = Cell::new(0);
    assert_eq!(run(program(&dropped)), Err("failed"));
    assert_eq!(dropped.get(), 1);
}
