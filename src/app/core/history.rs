use dissimilar::Chunk;

/// Holds the state of the File History
#[derive(Default)]
pub struct HistoryState {
    pub history_base: String,
    pub history_patches: Vec<Vec<dissimilar::Chunk<'static>>>,
    pub history_index: usize,
    pub cursor_before: Vec<(usize, usize)>, // cursor before patch[i] was applied
    pub cursor_after: Vec<(usize, usize)>,  // cursor after patch[i] was applied
    /// `history_index` at the last save, `None` if that state is no longer reachable
    pub saved_index: Option<usize>,
}

impl HistoryState {
    pub fn new_with_content(content: String) -> Self {
        Self {
            history_base: content,
            history_patches: Vec::new(),
            history_index: 0,
            cursor_before: Vec::new(),
            cursor_after: Vec::new(),
            saved_index: Some(0),
        }
    }

    /// Remembers the current position as the one written to disk
    pub fn mark_saved(&mut self) {
        self.saved_index = Some(self.history_index);
    }

    /// Whether the text currently shown is the one written to disk
    pub fn is_at_saved_state(&self) -> bool {
        self.saved_index == Some(self.history_index)
    }
}

/// Compute a patch (list of chunks) from `old` → `new`
pub fn make_patch(old: &str, new: &str) -> Vec<Chunk<'static>> {
    dissimilar::diff(old, new)
        .into_iter()
        .map(|chunk| match chunk {
            Chunk::Equal(s) => Chunk::Equal(Box::leak(s.to_string().into_boxed_str())),
            Chunk::Insert(s) => Chunk::Insert(Box::leak(s.to_string().into_boxed_str())),
            Chunk::Delete(s) => Chunk::Delete(Box::leak(s.to_string().into_boxed_str())),
        })
        .collect()
}

/// Apply a forward patch to `base`, reconstructing the text at that snapshot.
pub fn apply_patch(base: &str, patches: &[Vec<Chunk<'static>>]) -> String {
    patches.iter().fold(base.to_string(), |current, patch| {
        apply_single(&current, patch)
    })
}

pub fn apply_single(text: &str, patch: &[Chunk<'static>]) -> String {
    let mut result = String::with_capacity(text.len());
    for chunk in patch {
        match chunk {
            Chunk::Equal(s) | Chunk::Insert(s) => result.push_str(s),
            Chunk::Delete(_) => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opened_content_starts_at_the_saved_state() {
        let history = HistoryState::new_with_content("hello".into());
        assert!(history.is_at_saved_state());
    }

    #[test]
    fn a_never_saved_history_has_no_saved_state() {
        let history = HistoryState::default();
        assert!(!history.is_at_saved_state());
    }

    #[test]
    fn mark_saved_follows_the_current_index() {
        let mut history = HistoryState::new_with_content("hello".into());
        history.history_patches.push(make_patch("hello", "hello world"));
        history.history_index = 1;
        assert!(!history.is_at_saved_state());

        history.mark_saved();
        assert!(history.is_at_saved_state());

        history.history_index = 0;
        assert!(!history.is_at_saved_state());
    }
}
