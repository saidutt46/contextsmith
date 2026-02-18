pub mod collect;
pub mod diff;
pub mod explain;
pub mod init;
pub mod pack;

use crate::error::{ContextSmithError, Result};

/// Stub for unimplemented commands.
pub fn not_implemented(command: &str) -> Result<()> {
    Err(ContextSmithError::not_implemented(command))
}
