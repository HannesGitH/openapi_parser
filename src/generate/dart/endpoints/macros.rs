
#[macro_export]
macro_rules! cpf {
    ($content:expr, $($arg:tt)*) => {
        $content.push_str(&format!("{}\n", format!($($arg)*)));
    }
}