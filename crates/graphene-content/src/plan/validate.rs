use crate::plan::model::{
    CONTENT_PLAN_SCHEMA_VERSION, ContentMutationPlan, MAX_PLAN_ACTIONS, MAX_PLAN_ENTRIES,
    MAX_PLAN_FILESYSTEM_ACTIONS,
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::collections::HashSet;

impl ContentMutationPlan {
    /// Validates the consistency, security invariants, and resource limits of the mutation plan.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != CONTENT_PLAN_SCHEMA_VERSION {
            return Err(GrapheneError::new(
                ErrorCode::ContentPlanInvalid,
                ErrorKind::Content,
                "unsupported content mutation plan schema version",
            ));
        }

        if self.requested_actions.len() > MAX_PLAN_ACTIONS {
            return Err(GrapheneError::new(
                ErrorCode::ContentPlanInvalid,
                ErrorKind::Content,
                format!("requested actions exceed maximum limit of {MAX_PLAN_ACTIONS}"),
            ));
        }

        if self.planned_entries.len() > MAX_PLAN_ENTRIES {
            return Err(GrapheneError::new(
                ErrorCode::ContentPlanInvalid,
                ErrorKind::Content,
                format!("planned entries exceed maximum limit of {MAX_PLAN_ENTRIES}"),
            ));
        }

        if self.filesystem_actions.len() > MAX_PLAN_FILESYSTEM_ACTIONS {
            return Err(GrapheneError::new(
                ErrorCode::ContentPlanInvalid,
                ErrorKind::Content,
                format!(
                    "planned filesystem actions exceed maximum limit of {MAX_PLAN_FILESYSTEM_ACTIONS}"
                ),
            ));
        }

        let mut seen_entry_ids = HashSet::new();
        let mut seen_destinations = HashSet::new();
        let mut lowercase_destinations = HashSet::new();

        for entry in &self.planned_entries {
            if entry.entry_id.as_str().is_empty() {
                return Err(GrapheneError::new(
                    ErrorCode::ContentPlanInvalid,
                    ErrorKind::Content,
                    "content entry id cannot be empty",
                ));
            }

            if !seen_entry_ids.insert(entry.entry_id.as_str()) {
                return Err(GrapheneError::new(
                    ErrorCode::ContentPlanInvalid,
                    ErrorKind::Content,
                    format!("duplicate planned entry id: {}", entry.entry_id),
                ));
            }

            let dest_str = entry.destination.as_str();
            if !dest_str.starts_with(".minecraft/mods/") {
                return Err(GrapheneError::new(
                    ErrorCode::ContentPlanInvalid,
                    ErrorKind::Content,
                    format!("invalid destination path for mod content: {dest_str}"),
                ));
            }

            if !seen_destinations.insert(dest_str) {
                return Err(GrapheneError::new(
                    ErrorCode::ContentPlanInvalid,
                    ErrorKind::Content,
                    format!("duplicate planned destination path: {dest_str}"),
                ));
            }

            let lower_dest = dest_str.to_lowercase();
            if !lowercase_destinations.insert(lower_dest) {
                return Err(GrapheneError::new(
                    ErrorCode::ContentPlanInvalid,
                    ErrorKind::Content,
                    format!("case-folding destination collision detected for path: {dest_str}"),
                ));
            }
        }

        // Validate artifacts to acquire
        for artifact in &self.artifacts_to_acquire {
            if !artifact.is_verifiable() {
                return Err(GrapheneError::new(
                    ErrorCode::ContentFileUnverifiable,
                    ErrorKind::Content,
                    format!(
                        "artifact {} does not satisfy download integrity verification requirements",
                        artifact.file_ref
                    ),
                ));
            }
        }

        Ok(())
    }
}
