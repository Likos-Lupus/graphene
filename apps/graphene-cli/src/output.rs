use serde::Serialize;

/// Presentation mode selected by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
}

impl OutputMode {
    #[must_use]
    pub const fn from_json_flag(json: bool) -> Self {
        if json { Self::Json } else { Self::Human }
    }

    #[must_use]
    pub const fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

/// A command result carrying both a human presentation and a machine payload.
#[derive(Debug)]
pub struct Rendered {
    pub human: Vec<String>,
    pub json: serde_json::Value,
}

impl Rendered {
    #[must_use]
    pub fn new(human: Vec<String>, json: serde_json::Value) -> Self {
        Self { human, json }
    }

    /// Serializes a Graphene-owned value for machine output.
    #[must_use]
    pub fn value<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
    }
}

/// Prints a successful command result in the selected mode.
pub fn print_success(mode: OutputMode, rendered: &Rendered) {
    match mode {
        OutputMode::Human => {
            for line in &rendered.human {
                println!("{line}");
            }
        }
        OutputMode::Json => {
            let envelope = serde_json::json!({ "ok": true, "data": rendered.json });
            println!("{}", pretty(&envelope));
        }
    }
}

/// Prints a safe error envelope. Human mode never parses prose to discover codes.
pub fn print_error(mode: OutputMode, body: &crate::error::ErrorBody) {
    match mode {
        OutputMode::Human => {
            eprintln!("{}: {}", body.code, body.message);
            for (key, value) in &body.context {
                eprintln!("  {key}: {value}");
            }
        }

        OutputMode::Json => {
            let envelope = serde_json::json!({ "ok": false, "error": body });
            eprintln!("{}", pretty(&envelope));
        }
    }
}

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_owned())
}
