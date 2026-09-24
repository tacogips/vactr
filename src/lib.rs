//! nagamu - Nagamu scripting language
//!
//! This crate provides the core functionality for the nagamu project.

/// A placeholder function that returns a greeting message.
///
/// # Examples
///
/// ```
/// use nagamu::hello;
/// assert_eq!(hello(), "Hello from nagamu!");
/// ```
pub fn hello() -> &'static str {
    "Hello from nagamu!"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello(), "Hello from nagamu!");
    }
}
