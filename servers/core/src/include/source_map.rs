use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast::LineSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub(crate) usize);

impl FileId {
    pub fn new_dummy() -> Self {
        FileId(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSpan {
    pub file: FileId,
    pub lines: LineSpan,
}

#[derive(Debug, Clone)]
pub struct SourceMap {
    files: Vec<(PathBuf, String)>,
    path_to_id: HashMap<PathBuf, FileId>,
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceMap {
    pub fn new() -> Self {
        SourceMap {
            files: Vec::new(),
            path_to_id: HashMap::new(),
        }
    }

    pub fn register(&mut self, path: PathBuf, text: String) -> FileId {
        if let Some(&id) = self.path_to_id.get(&path) {
            return id;
        }
        let id = FileId(self.files.len());
        self.files.push((path.clone(), text));
        self.path_to_id.insert(path, id);
        id
    }

    pub fn get_path(&self, id: FileId) -> Option<&PathBuf> {
        self.files.get(id.0).map(|(p, _)| p)
    }

    pub fn get_text(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0).map(|(_, t)| t.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_new_file_returns_id() {
        let mut sm = SourceMap::new();
        let id = sm.register(PathBuf::from("/a.cir"), "R1 1 2 100".into());
        assert_eq!(sm.get_path(id).unwrap(), &PathBuf::from("/a.cir"));
    }

    #[test]
    fn test_register_same_path_twice_returns_same_id() {
        let mut sm = SourceMap::new();
        let id1 = sm.register(PathBuf::from("/a.cir"), "R1 1 2 100".into());
        let id2 = sm.register(PathBuf::from("/a.cir"), "different".into());
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_different_paths_get_different_ids() {
        let mut sm = SourceMap::new();
        let id1 = sm.register(PathBuf::from("/a.cir"), "R1".into());
        let id2 = sm.register(PathBuf::from("/b.cir"), "R2".into());
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_file_span_carries_both_file_id_and_line_range() {
        let mut sm = SourceMap::new();
        let id = sm.register(PathBuf::from("/a.cir"), "R1".into());
        let span = FileSpan {
            file: id,
            lines: 1..3,
        };
        assert_eq!(span.file, id);
        assert_eq!(span.lines, 1..3);
    }
}
