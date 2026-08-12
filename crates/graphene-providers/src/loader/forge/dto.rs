use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub(super) struct PromotionsDto {
    #[serde(default)]
    pub promos: BTreeMap<String, String>,
}
