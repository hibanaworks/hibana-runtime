// Small fixtures for polling the production row traversal directly. Macro
// tests exercise the balanced storage emitted for application declarations.
macro_rules! rows {
    () => { () };
    ($first:expr $(, $rest:expr)* $(,)?) => {
        hibana_runtime::__private::Pair {
            left: hibana_runtime::__private::Leaf { future: Some($first) },
            right: rows!($($rest),*),
        }
    };
}
macro_rules! runtime {
    ($error:ty; $($future:expr),* $(,)?) => {
        hibana_runtime::__private::Runtime::<$error, _>::new(rows!($($future),*))
    };
}
