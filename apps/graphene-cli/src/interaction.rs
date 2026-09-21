use crate::output::OutputMode;
use graphene::AuthInteraction;
use serde_json::json;

/// Emits the minimum intended authentication interaction to the interactive terminal or a
/// machine-readable stderr event. These transient values are never written to tracing output.
pub fn emit(mode: OutputMode, interaction: &AuthInteraction) {
    if let AuthInteraction::DeviceAuthorization {
        verification_uri,
        user_code,
        expires_in_seconds,
        poll_interval_seconds,
        ..
    } = interaction
    {
        let code = user_code.expose_secret();
        match mode {
            OutputMode::Human => {
                eprintln!(
                    "Open {verification_uri} and enter code {code} \
                     (expires in {expires_in_seconds}s, polling every {poll_interval_seconds}s)"
                );
            }
            OutputMode::Json => {
                let event = json!({
                    "event": "auth-interaction",
                    "kind": "device-authorization",
                    "verification_uri": verification_uri,
                    "user_code": code,
                    "expires_in_seconds": expires_in_seconds,
                    "poll_interval_seconds": poll_interval_seconds,
                });
                eprintln!("{event}");
            }
        }
    }
}
