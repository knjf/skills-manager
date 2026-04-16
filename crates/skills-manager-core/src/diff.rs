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

        let header = format!(
            "@@ -{},{} +{},{} @@",
            old_start + 1,
            old_end - old_start,
            new_start + 1,
            new_end - new_start,
        );

        let mut lines: Vec<DiffLine> = Vec::new();
        for op in &group {
            for change in diff.iter_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => DiffLineKind::Context,
                    ChangeTag::Insert => DiffLineKind::Added,
                    ChangeTag::Delete => DiffLineKind::Removed,
                };
                let text = change.to_string();
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
        assert!(hunks[0]
            .lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Added && l.text.trim() == "c"));
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
    fn empty_old_adds_all_new_lines() {
        let hunks = compute_diff("", "one\ntwo\n", 3);
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0]
            .lines
            .iter()
            .all(|l| l.kind == DiffLineKind::Added || l.kind == DiffLineKind::Context));
    }
}
