use crate::Dictionary;

/// A conflict resolution must refer to exactly what the user previewed.
pub struct Preview {
    pub local: Dictionary,
    pub remote: Dictionary,
    pub sha: Option<String>,
}

impl Preview {
    pub fn check(
        &self,
        local: &Dictionary,
        remote: &Dictionary,
        sha: &Option<String>,
    ) -> Result<(), String> {
        if self.local != *local || self.remote != *remote || self.sha != *sha {
            return Err("preview_expired".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_revision_invalidates_conflict_choice_even_if_content_matches() {
        let preview = Preview {
            local: Dictionary::default(),
            remote: Dictionary::default(),
            sha: Some("old".into()),
        };
        assert_eq!(
            preview.check(
                &Dictionary::default(),
                &Dictionary::default(),
                &Some("new".into())
            ),
            Err("preview_expired".into())
        );
    }
}
