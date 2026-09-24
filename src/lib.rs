//! vactrol - Vactrol scripting language
//!
//! This crate provides the core functionality for the vactrol project.

/// A placeholder function that returns a greeting message.
///
/// # Examples
///
/// ```
/// use vactrol::hello;
/// assert_eq!(hello(), "Hello from vactrol!");
/// ```
pub fn hello() -> &'static str {
    "Hello from vactrol!"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello(), "Hello from vactrol!");
    }
}
