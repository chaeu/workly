//! All file logic for Workly workspaces.

pub mod frontmatter;
pub mod model;
pub mod patch;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
