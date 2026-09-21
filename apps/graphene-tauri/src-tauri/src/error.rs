use graphene::GrapheneError;
use serde::Serialize;
use std::collections::BTreeMap;

/// Safe, serializable error envelope returned by every Tauri command.
///
/// It carries only stable Graphene code/kind/context and a developer message. Internal source
/// errors, absolute secret paths, and implementation handles never cross IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HostError {
    pub code: String,
    pub kind: String,
    pub message: String,
    pub context: BTreeMap<String, String>,
}

impl HostError {
    #[must_use]
    pub fn from_error(error: &GrapheneError) -> Self {
        Self {
            code: error.code.as_str().to_owned(),
            kind: format!("{:?}", error.kind),
            message: error.message().to_owned(),
            context: error
                .context
                .iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        }
    }

    #[must_use]
    pub fn invalid(field: &str, value: &str) -> Self {
        let mut context = BTreeMap::new();
        context.insert(field.to_owned(), value.to_owned());
        Self {
            code: "CONFIG_INVALID".to_owned(),
            kind: "Configuration".to_owned(),
            message: format!("invalid {field}"),
            context,
        }
    }
}

impl From<GrapheneError> for HostError {
    fn from(error: GrapheneError) -> Self {
        Self::from_error(&error)
    }
}
