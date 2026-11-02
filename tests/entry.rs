use core::{cell::Cell, task::Waker};
#[path = "support/driver.rs"]
mod driver;

#[hibana_runtime::main(driver::TestDriver(Waker::noop(), || {}))]
#[test]
async fn a_unit_test_entry_drives_its_children_and_keeps_unit_output() {
    let done = Cell::new(false);
    async {
        hibana_runtime::yield_now().await;
        done.set(true);
    };
}

#[test]
fn a_unit_entry_finishes_children_before_returning_to_its_caller() {
    #[hibana_runtime::main(driver::TestDriver(Waker::noop(), || {}))]
    async fn run(done: &Cell<bool>) {
        async {
            hibana_runtime::yield_now().await;
            done.set(true);
        };
    }
    let done = Cell::new(false);
    let (): () = run(&done);
    assert!(done.get());
}

#[test]
fn a_unit_early_return_waits_for_declared_children() {
    #[hibana_runtime::main(driver::TestDriver(Waker::noop(), || {}))]
    async fn run(done: &Cell<bool>, subsequent: &Cell<bool>) {
        async {
            hibana_runtime::yield_now().await;
            done.set(true);
        };
        if !done.get() {
            return;
        }
        subsequent.set(true);
    }
    let done = Cell::new(false);
    let subsequent = Cell::new(false);
    run(&done, &subsequent);
    assert!(done.get());
    assert!(!subsequent.get());
}

#[test]
fn a_unit_entry_preserves_nested_return_boundaries_and_tail_effects() {
    #[hibana_runtime::main(driver::TestDriver(Waker::noop(), || {}))]
    async fn run(answer: &Cell<u8>) {
        let nested = async {
            if answer.get() == 0 {
                return 7;
            }
            6
        };
        let nested_fn = || {
            if answer.get() == 0 {
                return 8;
            }
            9
        };
        answer.set(nested.await + nested_fn())
    }
    let answer = Cell::new(0);
    run(&answer);
    assert_eq!(answer.get(), 15);
}

#[test]
fn a_return_inside_a_unit_return_expression_keeps_its_original_boundary() {
    #[hibana_runtime::main(driver::TestDriver(Waker::noop(), || {}))]
    async fn run(done: &Cell<bool>, subsequent: &Cell<bool>) {
        async {
            hibana_runtime::yield_now().await;
            done.set(true);
        };
        return {
            if !done.get() {
                return;
            }
            subsequent.set(true);
        };
    }
    let done = Cell::new(false);
    let subsequent = Cell::new(false);
    run(&done, &subsequent);
    assert!(done.get());
    assert!(!subsequent.get());
}
