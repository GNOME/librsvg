//! Tracks metadata for a loading/rendering session.

use crate::log;

/// Metadata for a loading/rendering session.
///
/// When the calling program first uses one of the API entry points (e.g. `Loader::new()`
/// in the Rust API, or `rsvg_handle_new()` in the C API), there is no context yet where
/// librsvg's code may start to track things.  This struct provides that context.
pub struct Session {
    log_enabled: bool,
}

impl Session {
    pub fn new() -> Self {
        Self {
            log_enabled: log::log_enabled(),
        }
    }
}
