use core::future::Future;

use crate::executor::Leaf;

pub struct StoreResult<'a, F, V> {
    pub(crate) future: F,
    pub(crate) output: &'a mut Option<V>,
}

impl<F, V, E> core::future::Future for StoreResult<'_, F, V>
where
    F: core::future::Future<Output = Result<V, E>>,
{
    type Output = Result<(), E>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        // SAFETY: future is a private structurally pinned field. The separate
        // output reference is unpinned, and neither field is moved here. There
        // is no custom Drop or Unpin implementation.
        let (future, output) = unsafe {
            let store = self.get_unchecked_mut();
            (
                core::pin::Pin::new_unchecked(&mut store.future),
                &mut store.output,
            )
        };
        future.poll(cx).map(|result| {
            result.map(|value| {
                **output = Some(value);
            })
        })
    }
}

/// Slots contain ordinary unpinned output data, borrowed only while the
/// enclosing declaration drives its inputs. No result receiver is created.
pub trait Values {
    type Value;
    fn take(self) -> Self::Value;
}

impl<T> Values for Option<T> {
    type Value = T;

    fn take(self) -> T {
        self.expect("a completed input produced its value")
    }
}

impl<S: Values, const N: usize> Values for [S; N] {
    type Value = [S::Value; N];

    fn take(self) -> Self::Value {
        self.map(Values::take)
    }
}

/// Shape is inferred: a single future uses (), and a fixed array recursively
/// uses [Shape; N]. Tuple patterns are handled by Rust destructuring in the macro.
pub trait Inputs<'a, Shape>: Sized {
    type Slots: Values + 'a;
    type Rows;

    fn slots(&self) -> Self::Slots;
    fn rows(self, slots: &'a mut Self::Slots) -> Self::Rows;
}

impl<'a, F, T: 'a, E> Inputs<'a, ()> for F
where
    F: Future<Output = Result<T, E>>,
{
    type Slots = Option<T>;
    type Rows = Leaf<StoreResult<'a, F, T>>;

    fn slots(&self) -> Self::Slots {
        None
    }

    fn rows(self, output: &'a mut Self::Slots) -> Self::Rows {
        Leaf {
            future: Some(StoreResult {
                future: self,
                output,
            }),
        }
    }
}

impl<'a, I, Shape, const N: usize> Inputs<'a, [Shape; N]> for [I; N]
where
    I: Inputs<'a, Shape>,
{
    type Slots = [I::Slots; N];
    type Rows = [I::Rows; N];

    fn slots(&self) -> Self::Slots {
        core::array::from_fn(|index| self[index].slots())
    }

    fn rows(self, slots: &'a mut Self::Slots) -> Self::Rows {
        let mut outputs = slots.iter_mut();
        self.map(|input| input.rows(outputs.next().expect("one output slot per input")))
    }
}

pub fn input_slots<'a, I: Inputs<'a, Shape>, Shape>(input: &I) -> I::Slots {
    input.slots()
}

pub fn input_rows<'a, I: Inputs<'a, Shape>, Shape>(input: I, slots: &'a mut I::Slots) -> I::Rows {
    input.rows(slots)
}

pub fn take_values<S: Values>(slots: S) -> S::Value {
    slots.take()
}
