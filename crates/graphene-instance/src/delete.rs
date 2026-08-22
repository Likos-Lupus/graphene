use serde::{Deserialize, Serialize};

/// Options for deleting an instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeleteOptions {}
