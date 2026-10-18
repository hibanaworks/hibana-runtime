use core::task::Waker;
use hibana_runtime::Driver;

pub struct TestDriver<'a, F>(pub &'a Waker, pub F);

impl<F: FnMut()> Driver for TestDriver<'_, F> {
    fn waker(&self) -> Waker {
        self.0.clone()
    }

    fn wait(&mut self) {
        (self.1)();
    }
}
