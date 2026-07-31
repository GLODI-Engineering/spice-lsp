//! Dialect-specific search order for resolving an `.include`/`.lib`
//! filename to an actual path on disk (or in a [`FileSystem`] test double).
//! Entry point: [`resolve_include_path`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::dialect::Dialect;

/// An `.include`/`.lib` filename that couldn't be resolved to a real path
/// under any of the dialect's search locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathDiagnostic {
    /// A human-readable message naming the unresolvable file.
    pub message: String,
    /// The filename as written in the netlist (quotes stripped).
    pub filename: String,
    /// Every candidate path that was tried, in search order — useful for
    /// showing the user why resolution failed.
    pub tried: Vec<String>,
}

/// Filesystem abstraction so include resolution can be tested without
/// touching real disk. Implement this for a real filesystem in an embedder;
/// [`FakeFileSystem`] is the in-memory implementation used by this crate's
/// own tests.
pub trait FileSystem {
    /// Returns whether `path` exists.
    fn exists(&self, path: &Path) -> bool;
    /// Reads the full contents of `path` as a UTF-8 string.
    fn read_to_string(&self, path: &Path) -> std::io::Result<String>;
}

/// An in-memory [`FileSystem`] for tests: a fixed map of paths to file
/// contents, populated with [`FakeFileSystem::insert`].
pub struct FakeFileSystem {
    files: HashMap<PathBuf, String>,
}

impl FakeFileSystem {
    /// Creates an empty fake filesystem with no files.
    pub fn new() -> Self {
        FakeFileSystem {
            files: HashMap::new(),
        }
    }

    /// Adds a file at `path` with the given `content`.
    pub fn insert(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.files.insert(path.into(), content.into());
    }
}

impl Default for FakeFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystem for FakeFileSystem {
    fn exists(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn read_to_string(&self, path: &Path) -> std::io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"))
    }
}

/// Resolve `raw_filename` (as written after `.include`/`.lib`, possibly
/// quoted) to an actual path, trying locations in the dialect's search
/// order:
///
/// - **ngspice**: relative to `including_file_dir`, then each entry in
///   `_sourcepath_entries` (the `.options searchdir=...`/`SOURCEPATH`
///   list).
/// - **Xyce**: relative to `including_file_dir`, then `top_level_dir` (the
///   directory of the top-level netlist), then `exec_dir` (the current
///   working directory).
///
/// Returns the first candidate that exists, or a [`PathDiagnostic`] listing
/// every path tried if none exist.
pub fn resolve_include_path(
    fs: &dyn FileSystem,
    including_file_dir: &Path,
    top_level_dir: &Path,
    exec_dir: &Path,
    raw_filename: &str,
    _sourcepath_entries: &[PathBuf],
    dialect: Dialect,
) -> Result<PathBuf, PathDiagnostic> {
    let filename = strip_quotes(raw_filename);
    let mut tried = Vec::new();

    match dialect {
        Dialect::Ngspice => {
            let relative = including_file_dir.join(&filename);
            tried.push(relative.display().to_string());
            if fs.exists(&relative) {
                return Ok(relative);
            }

            for entry in _sourcepath_entries {
                let candidate = entry.join(&filename);
                tried.push(candidate.display().to_string());
                if fs.exists(&candidate) {
                    return Ok(candidate);
                }
            }
        }
        Dialect::Xyce => {
            let relative = including_file_dir.join(&filename);
            tried.push(relative.display().to_string());
            if fs.exists(&relative) {
                return Ok(relative);
            }

            let top = top_level_dir.join(&filename);
            tried.push(top.display().to_string());
            if fs.exists(&top) {
                return Ok(top);
            }

            let exec = exec_dir.join(&filename);
            tried.push(exec.display().to_string());
            if fs.exists(&exec) {
                return Ok(exec);
            }
        }
    }

    Err(PathDiagnostic {
        message: format!("unresolvable include file: '{filename}'"),
        filename,
        tried,
    })
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_fs() -> FakeFileSystem {
        let mut fs = FakeFileSystem::new();
        fs.insert("/proj/sub/models.lib", "content");
        fs.insert("/proj/models.lib", "content");
        fs.insert("/cwd/models.lib", "content");
        fs
    }

    #[test]
    fn test_ngspice_resolves_relative_to_including_file() {
        let fs = setup_fs();
        let result = resolve_include_path(
            &fs,
            Path::new("/proj/sub"),
            Path::new("/proj"),
            Path::new("/cwd"),
            "models.lib",
            &[],
            Dialect::Ngspice,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("sub/models.lib"));
    }

    #[test]
    fn test_xyce_resolves_relative_to_including_file_first() {
        let fs = setup_fs();
        let result = resolve_include_path(
            &fs,
            Path::new("/proj/sub"),
            Path::new("/proj"),
            Path::new("/cwd"),
            "models.lib",
            &[],
            Dialect::Xyce,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("sub/models.lib"));
    }

    #[test]
    fn test_xyce_falls_back_to_top_level_netlist_dir() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/proj/models.lib", "content");
        let result = resolve_include_path(
            &fs,
            Path::new("/proj/sub"),
            Path::new("/proj"),
            Path::new("/cwd"),
            "models.lib",
            &[],
            Dialect::Xyce,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("proj/models.lib"));
    }

    #[test]
    fn test_xyce_falls_back_to_exec_dir_last() {
        let mut fs = FakeFileSystem::new();
        fs.insert("/cwd/models.lib", "content");
        let result = resolve_include_path(
            &fs,
            Path::new("/proj/sub"),
            Path::new("/proj"),
            Path::new("/cwd"),
            "models.lib",
            &[],
            Dialect::Xyce,
        );
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("cwd/models.lib"));
    }

    #[test]
    fn test_unresolvable_filename_lists_all_tried_locations() {
        let fs = FakeFileSystem::new();
        let result = resolve_include_path(
            &fs,
            Path::new("/proj/sub"),
            Path::new("/proj"),
            Path::new("/cwd"),
            "missing.lib",
            &[],
            Dialect::Xyce,
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.tried.is_empty());
    }

    #[test]
    fn test_quoted_filename_strips_quotes_before_resolution() {
        let fs = setup_fs();
        let result = resolve_include_path(
            &fs,
            Path::new("/proj/sub"),
            Path::new("/proj"),
            Path::new("/cwd"),
            "\"models.lib\"",
            &[],
            Dialect::Ngspice,
        );
        assert!(result.is_ok());
    }
}
