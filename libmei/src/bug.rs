/// Like the `panic!` macro, but used to indicate bug
#[macro_export]
macro_rules! bug {
    ($($arg:tt)*) => (panic!("{}", format_args!($($arg)*)));
}
