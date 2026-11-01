use core::{cell::Cell, future::Future, task::Waker};
use hibana_runtime::{__private::block_on, scope};
#[path = "support/driver.rs"]
mod driver;
use driver::TestDriver;

type Outcome<T = ()> = Result<T, &'static str>;

fn run<F: Future>(future: F) -> F::Output {
    block_on(future, TestDriver(Waker::noop(), || {}))
}

struct Item<'a>(&'a Cell<u64>, u64);

impl Drop for Item<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() * 10 + self.1);
    }
}

fn pairs(drops: &Cell<u64>) -> [(Item<'_>, Item<'_>); 2] {
    [
        (Item(drops, 1), Item(drops, 2)),
        (Item(drops, 3), Item(drops, 4)),
    ]
}

fn ordinary_drops() -> u64 {
    let drops = Cell::new(0);
    for (_first, _second) in pairs(&drops) {}
    drops.get()
}

#[test]
fn disabled_array_children_keep_input_evaluation_and_pattern_drops() {
    #[scope]
    async fn program(calls: &Cell<u32>, drops: &Cell<u64>, effects: &Cell<u32>) -> Outcome<u64> {
        for (_first, _second) in {
            calls.set(calls.get() + 1);
            pairs(drops)
        } {
            #[cfg(any())]
            async move {
                effects.set(effects.get() + 1);
                drop(_second);
            };
        }
        Ok(drops.get())
    }
    let (calls, drops, effects) = (Cell::new(0), Cell::new(0), Cell::new(0));
    assert_eq!(run(program(&calls, &drops, &effects)), Ok(ordinary_drops()));
    assert_eq!(calls.get(), 1);
    assert_eq!(effects.get(), 0);
}

#[test]
fn disabled_collected_array_has_no_child_values() {
    #[scope]
    async fn program(calls: &Cell<u32>, drops: &Cell<u64>, effects: &Cell<u32>) -> Outcome {
        let values = async {
            for (_first, _second) in {
                calls.set(calls.get() + 1);
                pairs(drops)
            } {
                #[cfg(any())]
                async move {
                    effects.set(effects.get() + 1);
                    drop(_first);
                    drop(_second);
                    Ok(17u8)
                };
            }
        }
        .await?;
        assert_eq!(core::mem::size_of_val(&values), 0);
        assert_eq!(drops.get(), ordinary_drops());
        Ok(())
    }
    let (calls, drops, effects) = (Cell::new(0), Cell::new(0), Cell::new(0));
    assert_eq!(run(program(&calls, &drops, &effects)), Ok(()));
    assert_eq!(calls.get(), 1);
    assert_eq!(effects.get(), 0);
}

#[test]
fn enabled_array_children_return_values_after_pending() {
    #[scope]
    async fn program(effects: &Cell<u32>) -> Outcome<[u32; 2]> {
        for value in [3, 7] {
            #[cfg(all())]
            async move {
                hibana_runtime::yield_now().await;
                effects.set(effects.get() + 1);
                Ok(value)
            };
        }
    }
    let effects = Cell::new(0);
    assert_eq!(run(program(&effects)), Ok([3, 7]));
    assert_eq!(effects.get(), 2);
}

#[test]
fn cfg_attr_can_disable_discarded_array_children() {
    #[scope]
    async fn program(calls: &Cell<u32>, effects: &Cell<u32>) -> Outcome {
        for _value in {
            calls.set(calls.get() + 1);
            [3, 7]
        } {
            #[cfg_attr(all(), cfg(any()))]
            async move {
                effects.set(effects.get() + 1);
            };
        }
        Ok(())
    }
    let (calls, effects) = (Cell::new(0), Cell::new(0));
    assert_eq!(run(program(&calls, &effects)), Ok(()));
    assert_eq!(calls.get(), 1);
    assert_eq!(effects.get(), 0);
}

#[test]
fn nested_cfg_attr_can_disable_collected_array_children() {
    #[scope]
    async fn program(calls: &Cell<u32>, effects: &Cell<u32>) -> Outcome {
        let values = async {
            for _value in {
                calls.set(calls.get() + 1);
                [3, 7]
            } {
                #[cfg_attr(all(), cfg_attr(all(), cfg(any())))]
                async move {
                    effects.set(effects.get() + 1);
                    Ok(17u8)
                };
            }
        }
        .await?;
        assert_eq!(core::mem::size_of_val(&values), 0);
        Ok(())
    }
    let (calls, effects) = (Cell::new(0), Cell::new(0));
    assert_eq!(run(program(&calls, &effects)), Ok(()));
    assert_eq!(calls.get(), 1);
    assert_eq!(effects.get(), 0);
}

#[test]
fn inactive_cfg_attr_preserves_array_values() {
    #[scope]
    async fn program() -> Outcome<[u32; 2]> {
        for value in [3, 7] {
            #[cfg_attr(any(), cfg(any()))]
            async move {
                Ok(value)
            };
        }
    }
    assert_eq!(run(program()), Ok([3, 7]));
}

#[test]
fn enabled_cfg_attr_preserves_body_lints_and_values() {
    #[scope]
    async fn program() -> Outcome<[u32; 2]> {
        for value in [3, 7] {
            #[cfg_attr(all(), cfg(all()), allow(unused_mut))]
            async move {
                let mut result = value;
                Ok(result)
            };
        }
    }
    assert_eq!(run(program()), Ok([3, 7]));
}

#[test]
fn disabled_children_do_not_disable_the_awaited_array_input() {
    #[scope]
    async fn program(input_effects: &Cell<u32>, child_effects: &Cell<u32>) -> Outcome {
        for _value in (async {
            async {
                hibana_runtime::yield_now().await;
                input_effects.set(input_effects.get() + 1);
            };
            Ok([3, 7])
        })
        .await?
        {
            #[cfg(any())]
            async move {
                child_effects.set(child_effects.get() + 1);
            };
        }
        Ok(())
    }
    let (input_effects, child_effects) = (Cell::new(0), Cell::new(0));
    assert_eq!(run(program(&input_effects, &child_effects)), Ok(()));
    assert_eq!(input_effects.get(), 1);
    assert_eq!(child_effects.get(), 0);
}

#[test]
fn child_lint_expectations_are_applied_once_for_array_values() {
    #[scope]
    async fn program() -> Outcome<[u32; 2]> {
        for value in [3, 7] {
            #[cfg_attr(all(), expect(unused_mut))]
            async move {
                let mut result = value;
                Ok(result)
            };
        }
    }
    assert_eq!(run(program()), Ok([3, 7]));
}

#[test]
fn child_lint_expectations_are_applied_once_for_discarded_children() {
    #[scope]
    async fn program() -> Outcome {
        #[expect(unused_mut)]
        async {
            let mut result = 7;
            assert_eq!(result, 7);
        };
        Ok(())
    }
    assert_eq!(run(program()), Ok(()));
}
