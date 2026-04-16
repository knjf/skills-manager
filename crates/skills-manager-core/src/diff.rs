use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    /// Line content including its trailing newline if the original line had one.
    /// Last line of a file without a trailing newline will have no `\n` here.
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// Compute unified hunks between two texts.
/// `context` = number of unchanged lines around each change (e.g. 3).
pub fn compute_diff(old: &str, new: &str, context: usize) -> Vec<DiffHunk> {
    let diff = TextDiff::from_lines(old, new);
    let mut hunks: Vec<DiffHunk> = Vec::new();

    for group in diff.grouped_ops(context) {
        if group.is_empty() {
            continue;
        }

        let first = group.first().unwrap();
        let last = group.last().unwrap();
        let old_start = first.old_range().start;
        let old_end = last.old_range().end;
        let new_start = first.new_range().start;
        let new_end = last.new_range().end;

        let old_count = old_end - old_start;
        let new_count = new_end - new_start;
        let old_display_start = if old_count == 0 { 0 } else { old_start + 1 };
        let new_display_start = if new_count == 0 { 0 } else { new_start + 1 };
        let header = format!(
            "@@ -{},{} +{},{} @@",
            old_display_start, old_count, new_display_start, new_count,
        );

        let mut lines: Vec<DiffLine> = Vec::new();
        for op in &group {
            for change in diff.iter_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Insert => DiffLineKind::Added,
                    ChangeTag::Delete => DiffLineKind::Removed,
                };
                let text = change.value().to_string();
                lines.push(DiffLine {
                    kind,
                    old_no: change.old_index().map(|i| (i + 1) as u32),
                    new_no: change.new_index().map(|i| (i + 1) as u32),
                    text,
                });
            }
        }

        hunks.push(DiffHunk { header, lines });
    }

    hunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_inputs_yield_no_hunks() {
        let hunks = compute_diff("same\ntext\n", "same\ntext\n", 3);
        assert!(hunks.is_empty());
    }

    #[test]
    fn simple_addition_produces_one_hunk() {
        let hunks = compute_diff("a\nb\n", "a\nb\nc\n", 3);
        assert_eq!(hunks.len(), 1);
        let added = hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Added)
            .expect("must have an added line");
        assert_eq!(added.text.trim_end_matches('\n'), "c");
        assert_eq!(added.new_no, Some(3));
        assert_eq!(added.old_no, None);
    }

    #[test]
    fn replacement_shows_both_removed_and_added() {
        let hunks = compute_diff("hello\n", "world\n", 3);
        assert_eq!(hunks.len(), 1);
        let kinds: Vec<_> = hunks[0].lines.iter().map(|l| &l.kind).collect();
        assert!(kinds.contains(&&DiffLineKind::Removed));
        assert!(kinds.contains(&&DiffLineKind::Added));
    }

    #[test]
    fn empty_old_adds_all_new_lines_with_zero_header() {
        let hunks = compute_diff("", "one\ntwo\n", 3);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].header, "@@ -0,0 +1,2 @@");
        assert!(hunks[0].lines.iter().all(|l| l.kind == DiffLineKind::Added));
    }

    #[test]
    fn empty_new_header_uses_zero_sentinel() {
        let hunks = compute_diff("a\nb\n", "", 3);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].header, "@@ -1,2 +0,0 @@");
    }

    #[test]
    fn no_trailing_newline_not_duplicated() {
        let hunks = compute_diff("old", "new", 3);
        assert_eq!(hunks.len(), 1);
        let added = hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == DiffLineKind::Added)
            .unwrap();
        // Original "new" had no trailing newline; text must not fabricate one.
        assert_eq!(added.text, "new");
    }
}
