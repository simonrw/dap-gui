use std::{borrow::Cow, path::Path};

pub fn normalise_path(path: &Path) -> Cow<'_, Path> {
    // Try to expand tilde prefix to home directory
    if let Ok(stripped) = path.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return Cow::Owned(home.join(stripped));
        }
        // If home directory cannot be determined, log and return path as-is
        tracing::warn!("cannot determine home directory, using path as-is");
    }
    Cow::Borrowed(path)
}
