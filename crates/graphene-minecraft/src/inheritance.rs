pub const MAX_INHERITANCE_DEPTH: usize = 16;

use crate::{MinecraftVersionId, error::mc_error};
use graphene_core::{ErrorCode, Result};

/// Domain-owned bounded inheritance traversal state used by provider adapters while fetching a
/// parent chain. It prevents provider-specific recursion code from defining cycle/depth semantics.
#[derive(Debug, Default)]
pub struct InheritanceTracker {
    stack: Vec<MinecraftVersionId>,
}

impl InheritanceTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn enter(&mut self, id: &MinecraftVersionId) -> Result<()> {
        if self.stack.iter().any(|entry| entry == id) {
            return Err(mc_error(
                ErrorCode::MinecraftInheritanceCycle,
                "Minecraft metadata inheritance contains a cycle",
            )
            .with_context("version_id", id.to_string()));
        }

        if self.stack.len() >= MAX_INHERITANCE_DEPTH {
            return Err(mc_error(
                ErrorCode::MinecraftInheritanceTooDeep,
                "Minecraft metadata inheritance exceeds the configured maximum depth",
            )
            .with_context("version_id", id.to_string()));
        }

        self.stack.push(id.clone());
        Ok(())
    }

    pub fn leave(&mut self, id: &MinecraftVersionId) {
        let popped = self.stack.pop();
        debug_assert_eq!(popped.as_ref(), Some(id));
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inheritance_tracker_rejects_cycles_and_excessive_depth() {
        let mut tracker = InheritanceTracker::new();
        let root = MinecraftVersionId::new("root").expect("version");
        tracker.enter(&root).expect("enter");
        assert_eq!(
            tracker.enter(&root).expect_err("cycle").code,
            ErrorCode::MinecraftInheritanceCycle
        );
        tracker.leave(&root);

        let mut tracker = InheritanceTracker::new();
        let ids = (0..MAX_INHERITANCE_DEPTH)
            .map(|index| MinecraftVersionId::new(format!("v{index}")).expect("version"))
            .collect::<Vec<_>>();
        for id in &ids {
            tracker.enter(id).expect("enter");
        }

        let too_deep = MinecraftVersionId::new("too-deep").expect("version");
        assert_eq!(
            tracker.enter(&too_deep).expect_err("depth").code,
            ErrorCode::MinecraftInheritanceTooDeep
        );

        for id in ids.iter().rev() {
            tracker.leave(id);
        }
    }
}
