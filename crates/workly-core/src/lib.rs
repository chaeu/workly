//! All file logic for Workly workspaces. Filled from M1 on.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(super::VERSION, "0.1.0");
    }
}
