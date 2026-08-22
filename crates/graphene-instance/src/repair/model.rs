use crate::{
    InstanceStateFingerprint,
    repair::action::RepairAction,
    verification::{VerificationFinding, VerificationMode, VerificationReport},
};
use graphene_core::InstanceId;
use serde::{Deserialize, Serialize};

pub const REPAIR_PLAN_SCHEMA_VERSION: u32 = 1;

/// Options configuring repair planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairOptions {
    /// Verification mode to use when constructing the pre-repair diagnostic report.
    pub verification_mode: VerificationMode,
}

impl Default for RepairOptions {
    fn default() -> Self {
        Self {
            verification_mode: VerificationMode::Full,
        }
    }
}

/// Deterministic, non-mutating repair plan derived from durable desired state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairPlan {
    pub schema_version: u32,
    pub instance_id: InstanceId,
    pub base_state_fingerprint: InstanceStateFingerprint,
    pub verification_mode: VerificationMode,
    pub ordered_actions: Vec<RepairAction>,
    pub estimated_download_bytes: u64,
    pub requires_network: bool,
    pub requires_tool_java: bool,
    pub residual_diagnostics: Vec<VerificationFinding>,
}

impl RepairPlan {
    /// Returns whether this repair plan is a no-op (instance needs no repairs).
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.ordered_actions.is_empty()
    }
}

/// Result returned after executing a repair plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairResult {
    pub instance_id: InstanceId,
    pub executed_actions_count: usize,
    pub post_verify_report: VerificationReport,
}
