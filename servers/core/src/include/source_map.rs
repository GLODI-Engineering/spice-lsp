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
    // Multi-file merge support (see CORE-25): each entry reserves a
    // disjoint block of "virtual" line numbers for one file, so that
    // statement spans from different files can be merged into a single
    // Vec<Statement> (no new field on Statement/Scope needed at all) while
    // still being unambiguously traceable back to (FileId, real line).
    // Populated only by callers doing multi-file resolution
    // (include::graph); single-file callers never touch this and are
    // completely unaffected.
    offsets: Vec<(FileId, usize /* base */, usize /* line_count */)>,
    next_base: usize,
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
            offsets: Vec::new(),
            // Start at 1 so a virtual line number is never 0, keeping it
            // consistent with the 1-based line numbers used everywhere
            // else in this crate.
            next_base: 1,
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

    /// Reserve a disjoint block of `line_count` virtual line numbers for
    /// `file`, returning the base to add to that file's real line numbers.
    /// Idempotent per file: calling this twice for the same `file` returns
    /// the same base rather than reserving a second block.
    pub fn assign_offset(&mut self, file: FileId, line_count: usize) -> usize {
        if let Some(&(_, base, _)) = self.offsets.iter().find(|(f, _, _)| *f == file) {
            return base;
        }
        let base = self.next_base;
        self.offsets.push((file, base, line_count));
        self.next_base += line_count + 1;
        base
    }

    /// Translate a virtual line number (one previously produced by adding
    /// an `assign_offset` base to a real line number) back to the file it
    /// came from and the real line number within that file.
    pub fn resolve_virtual_line(&self, virtual_line: usize) -> Option<(FileId, usize)> {
        for &(file, base, line_count) in &self.offsets {
            if virtual_line >= base && virtual_line < base + line_count {
                return Some((file, virtual_line - base + 1));
            }
        }
        None
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

    #[test]
    fn test_assign_offset_reserves_disjoint_blocks() {
        let mut sm = SourceMap::new();
        let a = sm.register(PathBuf::from("/a.cir"), "line1\nline2".into());
        let b = sm.register(PathBuf::from("/b.cir"), "line1\nline2\nline3".into());

        let base_a = sm.assign_offset(a, 2);
        let base_b = sm.assign_offset(b, 3);

        assert!(
            base_b >= base_a + 2,
            "b's block must not overlap a's 2-line block: base_a={base_a} base_b={base_b}"
        );
    }

    #[test]
    fn test_assign_offset_idempotent_per_file() {
        let mut sm = SourceMap::new();
        let a = sm.register(PathBuf::from("/a.cir"), "line1".into());
        let base1 = sm.assign_offset(a, 1);
        let base2 = sm.assign_offset(a, 1);
        assert_eq!(base1, base2);
    }

    #[test]
    fn test_resolve_virtual_line_round_trips() {
        let mut sm = SourceMap::new();
        let a = sm.register(PathBuf::from("/a.cir"), "l1\nl2".into());
        let b = sm.register(PathBuf::from("/b.cir"), "l1\nl2\nl3".into());
        let base_a = sm.assign_offset(a, 2);
        let base_b = sm.assign_offset(b, 3);

        assert_eq!(sm.resolve_virtual_line(base_a), Some((a, 1)));
        assert_eq!(sm.resolve_virtual_line(base_a + 1), Some((a, 2)));
        assert_eq!(sm.resolve_virtual_line(base_b), Some((b, 1)));
        assert_eq!(sm.resolve_virtual_line(base_b + 2), Some((b, 3)));
    }

    #[test]
    fn test_resolve_virtual_line_out_of_range_returns_none() {
        let sm = SourceMap::new();
        assert_eq!(sm.resolve_virtual_line(999), None);
    }
}
