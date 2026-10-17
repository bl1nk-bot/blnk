//! Path handling utilities.

use std::path::{Path, PathBuf};

/// Canonicalizes a path and strips Windows verbatim `\\?\` prefix when present.
///
/// Standard `std::fs::canonicalize` on Windows returns UNC verbatim paths like `\\?\C:\foo`.
/// Prefixing breaks path prefix checks (`starts_with`) when comparing against non-verbatim paths.
pub fn canonicalize(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let p = std::fs::canonicalize(path)?;
    #[cfg(windows)]
    {
        let s = p.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            if !stripped.starts_with("UNC\\") {
                return Ok(PathBuf::from(stripped));
            }
        }
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_returns_valid_existing_path() {
        let temp = std::env::temp_dir();
        let canonical = canonicalize(&temp).expect("temp dir should canonicalize");
        assert!(canonical.is_dir());
    }
}
