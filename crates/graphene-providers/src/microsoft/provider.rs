use super::{config::MicrosoftAuthConfig, dto::*, policy::*};
use graphene_auth::{
    Account, AccountKind, AccountProfile, AuthChallenge, AuthFuture, AuthInteraction, AuthProvider,
    AuthProviderCapabilities, AuthSession, RefreshCredential,
};
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, OperationController, Result, SensitiveString,
};
use graphene_network::{BoundedResponse, NetworkClient};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

#[derive(Clone)]
pub struct MicrosoftAuthProvider {
    network: NetworkClient,
    config: MicrosoftAuthConfig,
}

impl std::fmt::Debug for MicrosoftAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosoftAuthProvider")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl MicrosoftAuthProvider {
    pub fn new(network: NetworkClient, config: MicrosoftAuthConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { network, config })
    }

    async fn begin_inner(&self, operation: &OperationController) -> Result<AuthChallenge> {
        operation.set_stage("request_device_code")?;
        checkpoint(operation)?;

        let scope = self.config.scope_string();
        let response = self
            .network
            .post_form_bounded(
                self.config.device_code_endpoint(),
                &[
                    ("client_id", self.config.client_id()),
                    ("scope", scope.as_str()),
                ],
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;
        if !(200..300).contains(&response.status) {
            return Err(provider_error(
                ErrorCode::AuthProviderUnavailable,
                "request_device_code",
                Some(response.status),
                "device authorization request was rejected",
            ));
        }

        let dto: DeviceCodeDto = parse(&response, "request_device_code")?;
        for value in [&dto.device_code, &dto.user_code, &dto.verification_uri] {
            validate_string(value, "request_device_code")?;
        }

        if let Some(message) = &dto.message {
            validate_string(message, "request_device_code")?;
        }

        validate_verification_uri(&dto.verification_uri, self.config.allow_http())?;
        if dto.expires_in == 0 || dto.expires_in > 86_400 {
            return Err(provider_error(
                ErrorCode::AuthProviderUnavailable,
                "request_device_code",
                None,
                "device authorization expiry is invalid",
            ));
        }

        let interval = dto.interval.unwrap_or(5).clamp(1, 60);
        operation.set_stage("await_user_authorization")?;
        Ok(AuthChallenge {
            interaction: AuthInteraction::DeviceAuthorization {
                verification_uri: dto.verification_uri,
                user_code: SensitiveString::new(dto.user_code),
                message: dto.message.map(SensitiveString::new),
                expires_in_seconds: dto.expires_in,
                poll_interval_seconds: interval,
            },
            provider_state: SensitiveString::new(dto.device_code),
            expires_at: Instant::now() + Duration::from_secs(dto.expires_in),
        })
    }

    async fn complete_inner(
        &self,
        challenge: AuthChallenge,
        operation: &OperationController,
    ) -> Result<AuthSession> {
        let mut interval = match &challenge.interaction {
            AuthInteraction::DeviceAuthorization {
                poll_interval_seconds,
                ..
            } => *poll_interval_seconds,
            _ => {
                return Err(provider_error(
                    ErrorCode::AuthRequestInvalid,
                    "await_user_authorization",
                    None,
                    "authentication interaction type is unsupported by the Microsoft adapter",
                ));
            }
        };
        let deadline = challenge.expires_at;
        let device_code = challenge.provider_state;
        let token = loop {
            checkpoint(operation)?;
            if Instant::now() >= deadline {
                return Err(provider_error(
                    ErrorCode::AuthInteractionExpired,
                    "await_user_authorization",
                    None,
                    "device authorization expired",
                ));
            }

            let cancellation = operation.cancellation_token();
            tokio::select! {
                () = cancellation.cancelled() => return Err(cancelled()),
                () = sleep(Duration::from_secs(interval)) => {}
            }

            checkpoint(operation)?;
            if Instant::now() >= deadline {
                return Err(provider_error(
                    ErrorCode::AuthInteractionExpired,
                    "await_user_authorization",
                    None,
                    "device authorization expired",
                ));
            }
            operation.set_stage("exchange_microsoft_token")?;

            let response = self
                .network
                .post_form_bounded(
                    self.config.token_endpoint(),
                    &[
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                        ("client_id", self.config.client_id()),
                        ("device_code", device_code.expose_secret()),
                    ],
                    MAX_AUTH_RESPONSE_BYTES,
                    self.config.allow_http(),
                    operation,
                )
                .await
                .map_err(map_transport)?;

            if (200..300).contains(&response.status) {
                break parse::<TokenDto>(&response, "exchange_microsoft_token")?;
            }

            let error = parse_oauth_error(&response)?;
            match classify_device_poll_error(&error) {
                DevicePollError::AuthorizationPending => {
                    operation.set_stage("await_user_authorization")?;
                }
                DevicePollError::SlowDown => {
                    interval = interval.saturating_add(5).min(60);
                    operation.set_stage("await_user_authorization")?;
                }
                DevicePollError::AuthorizationDeclined => {
                    return Err(provider_error(
                        ErrorCode::AuthDeviceAuthorizationRejected,
                        "await_user_authorization",
                        Some(response.status),
                        "device authorization was declined",
                    ));
                }
                DevicePollError::Expired => {
                    return Err(provider_error(
                        ErrorCode::AuthInteractionExpired,
                        "await_user_authorization",
                        Some(response.status),
                        "device authorization expired or became invalid",
                    ));
                }
                DevicePollError::InteractionRequired => {
                    return Err(provider_error(
                        ErrorCode::AuthInteractionRequired,
                        "await_user_authorization",
                        Some(response.status),
                        "authentication requires a new user interaction",
                    ));
                }
                DevicePollError::Other => {
                    return Err(provider_error(
                        ErrorCode::AuthProviderUnavailable,
                        "exchange_microsoft_token",
                        Some(response.status),
                        "identity token exchange failed",
                    ));
                }
            }
        };

        let refresh = token.refresh_token.ok_or_else(|| {
            provider_error(
                ErrorCode::AuthProviderUnavailable,
                "exchange_microsoft_token",
                None,
                "identity response omitted refresh credential",
            )
        })?;

        validate_string(&refresh, "exchange_microsoft_token")?;
        self.finish_chain(
            token.access_token,
            RefreshCredential::new(refresh),
            operation,
        )
        .await
    }

    async fn refresh_inner(
        &self,
        account: &Account,
        credential: &RefreshCredential,
        operation: &OperationController,
    ) -> Result<AuthSession> {
        if account.kind != AccountKind::Microsoft {
            return Err(provider_error(
                ErrorCode::AuthAccountStateInvalid,
                "refresh",
                None,
                "refresh requested for a non-Microsoft account",
            ));
        }

        checkpoint(operation)?;
        operation.set_stage("exchange_microsoft_token")?;
        let scope = self.config.scope_string();
        let response = self
            .network
            .post_form_bounded(
                self.config.token_endpoint(),
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", self.config.client_id()),
                    ("refresh_token", credential.expose_secret()),
                    ("scope", scope.as_str()),
                ],
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;

        if !(200..300).contains(&response.status) {
            let error = parse_oauth_error(&response).unwrap_or_default();
            return Err(match error.as_str() {
                "invalid_grant" => provider_error(
                    ErrorCode::AuthRefreshRejected,
                    "refresh",
                    Some(response.status),
                    "stored refresh credential was rejected",
                ),
                "interaction_required" => provider_error(
                    ErrorCode::AuthInteractionRequired,
                    "refresh",
                    Some(response.status),
                    "provider requires a new user interaction",
                ),
                _ => provider_error(
                    ErrorCode::AuthProviderUnavailable,
                    "refresh",
                    Some(response.status),
                    "identity refresh failed temporarily",
                ),
            });
        }

        let token: TokenDto = parse(&response, "refresh")?;
        let refresh = match token.refresh_token {
            Some(value) => {
                validate_string(&value, "refresh")?;
                RefreshCredential::new(value)
            }
            None => credential.clone(),
        };

        self.finish_chain(token.access_token, refresh, operation)
            .await
    }

    async fn finish_chain(
        &self,
        microsoft_access: String,
        refresh: RefreshCredential,
        operation: &OperationController,
    ) -> Result<AuthSession> {
        validate_string(&microsoft_access, "authenticate_xbox")?;

        checkpoint(operation)?;
        operation.set_stage("authenticate_xbox")?;
        let xbox_body = serde_json::to_vec(&json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={microsoft_access}") },
            "RelyingParty": self.config.services().xbox_relying_party,
            "TokenType": "JWT"
        })).map_err(|source| provider_error(ErrorCode::AuthXboxRejected, "authenticate_xbox", None, "failed to construct Xbox request").with_source(source))?;
        let xbox_response = self
            .network
            .post_json_bounded(
                self.config.services().xbox_user_auth.as_str(),
                &xbox_body,
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;

        if !(200..300).contains(&xbox_response.status) {
            return Err(provider_error(
                ErrorCode::AuthXboxRejected,
                "authenticate_xbox",
                Some(xbox_response.status),
                "Xbox authentication was rejected",
            ));
        }

        let xbox: XboxTokenDto = parse(&xbox_response, "authenticate_xbox")?;
        validate_string(&xbox.token, "authenticate_xbox")?;
        let claim = xbox.display_claims.xui.into_iter().next().ok_or_else(|| {
            provider_error(
                ErrorCode::AuthXboxRejected,
                "authenticate_xbox",
                None,
                "Xbox response omitted user hash",
            )
        })?;
        validate_string(&claim.uhs, "authenticate_xbox")?;

        checkpoint(operation)?;
        operation.set_stage("authorize_xsts")?;
        let xsts_body = serde_json::to_vec(&json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbox.token] },
            "RelyingParty": self.config.services().xsts_relying_party,
            "TokenType": "JWT"
        }))
        .map_err(|source| {
            provider_error(
                ErrorCode::AuthXstsRejected,
                "authorize_xsts",
                None,
                "failed to construct XSTS request",
            )
            .with_source(source)
        })?;
        let xsts_response = self
            .network
            .post_json_bounded(
                self.config.services().xsts_authorize.as_str(),
                &xsts_body,
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;

        if !(200..300).contains(&xsts_response.status) {
            return Err(provider_error(
                ErrorCode::AuthXstsRejected,
                "authorize_xsts",
                Some(xsts_response.status),
                "XSTS authorization was rejected",
            ));
        }

        let xsts: XboxTokenDto = parse(&xsts_response, "authorize_xsts")?;
        validate_string(&xsts.token, "authorize_xsts")?;
        let xsts_claim = xsts
            .display_claims
            .xui
            .into_iter()
            .next()
            .unwrap_or(XboxUserClaimDto {
                uhs: claim.uhs.clone(),
                xid: claim.xid.clone(),
            });
        let user_hash = if xsts_claim.uhs.is_empty() {
            claim.uhs
        } else {
            xsts_claim.uhs
        };
        let xuid = xsts_claim.xid.or(claim.xid);

        if let Some(value) = &xuid {
            validate_string(value, "authorize_xsts")?;
        }

        checkpoint(operation)?;
        operation.set_stage("authenticate_minecraft")?;
        let mc_body = serde_json::to_vec(
            &json!({ "identityToken": format!("XBL3.0 x={user_hash};{}", xsts.token) }),
        )
        .map_err(|source| {
            provider_error(
                ErrorCode::AuthMinecraftRejected,
                "authenticate_minecraft",
                None,
                "failed to construct Minecraft authentication request",
            )
            .with_source(source)
        })?;
        let mc_response = self
            .network
            .post_json_bounded(
                self.config.services().minecraft_login.as_str(),
                &mc_body,
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;

        if !(200..300).contains(&mc_response.status) {
            return Err(provider_error(
                ErrorCode::AuthMinecraftRejected,
                "authenticate_minecraft",
                Some(mc_response.status),
                "Minecraft services authentication was rejected",
            ));
        }

        let mc: MinecraftTokenDto = parse(&mc_response, "authenticate_minecraft")?;
        validate_string(&mc.access_token, "authenticate_minecraft")?;
        let mc_access = SensitiveString::new(mc.access_token);

        checkpoint(operation)?;
        operation.set_stage("verify_entitlement")?;
        let entitlement_response = self
            .network
            .get_bearer_bounded(
                self.config.services().minecraft_entitlements.as_str(),
                &mc_access,
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;
        if !(200..300).contains(&entitlement_response.status) {
            return Err(provider_error(
                ErrorCode::AuthMinecraftRejected,
                "verify_entitlement",
                Some(entitlement_response.status),
                "Minecraft entitlement request failed",
            ));
        }

        let entitlements: EntitlementsDto = parse(&entitlement_response, "verify_entitlement")?;
        if entitlements.items.is_empty() {
            return Err(provider_error(
                ErrorCode::AuthEntitlementMissing,
                "verify_entitlement",
                None,
                "Minecraft entitlement is missing",
            ));
        }
        let _ = entitlements.items.iter().any(|item| !item.name.is_empty());

        checkpoint(operation)?;
        operation.set_stage("fetch_profile")?;
        let profile_response = self
            .network
            .get_bearer_bounded(
                self.config.services().minecraft_profile.as_str(),
                &mc_access,
                MAX_AUTH_RESPONSE_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(map_transport)?;
        if profile_response.status == 404 {
            return Err(provider_error(
                ErrorCode::AuthProfileMissing,
                "fetch_profile",
                Some(404),
                "Minecraft profile is missing",
            ));
        }

        if !(200..300).contains(&profile_response.status) {
            return Err(provider_error(
                ErrorCode::AuthMinecraftRejected,
                "fetch_profile",
                Some(profile_response.status),
                "Minecraft profile request failed",
            ));
        }

        let profile: MinecraftProfileDto = parse(&profile_response, "fetch_profile")?;
        validate_string(&profile.name, "fetch_profile")?;
        let uuid = Uuid::parse_str(&profile.id).map_err(|source| {
            provider_error(
                ErrorCode::AuthProfileMissing,
                "fetch_profile",
                None,
                "Minecraft profile UUID is invalid",
            )
            .with_source(source)
        })?;

        Ok(AuthSession {
            kind: AccountKind::Microsoft,
            profile: AccountProfile {
                display_name: profile.name,
                minecraft_uuid: uuid,
            },
            access_token: mc_access,
            refresh_credential: Some(refresh),
            client_id: Some(SensitiveString::new(self.config.client_id())),
            xuid: xuid.map(SensitiveString::new),
        })
    }
}

impl AuthProvider for MicrosoftAuthProvider {
    fn capabilities(&self) -> AuthProviderCapabilities {
        AuthProviderCapabilities {
            device_authorization: true,
            refresh: true,
        }
    }

    fn begin<'a>(&'a self, operation: &'a OperationController) -> AuthFuture<'a, AuthChallenge> {
        Box::pin(self.begin_inner(operation))
    }

    fn complete<'a>(
        &'a self,
        challenge: AuthChallenge,
        operation: &'a OperationController,
    ) -> AuthFuture<'a, AuthSession> {
        Box::pin(self.complete_inner(challenge, operation))
    }

    fn refresh<'a>(
        &'a self,
        account: &'a Account,
        credential: &'a RefreshCredential,
        operation: &'a OperationController,
    ) -> AuthFuture<'a, AuthSession> {
        Box::pin(self.refresh_inner(account, credential, operation))
    }
}

fn parse<T: DeserializeOwned>(response: &BoundedResponse, stage: &'static str) -> Result<T> {
    serde_json::from_slice(&response.body).map_err(|source| {
        provider_error(
            ErrorCode::AuthProviderUnavailable,
            stage,
            Some(response.status),
            "authentication provider returned malformed JSON",
        )
        .with_source(source)
    })
}

fn parse_oauth_error(response: &BoundedResponse) -> Result<String> {
    let error = parse::<OAuthErrorDto>(response, "oauth_error")?.error;
    validate_string(&error, "oauth_error")?;
    Ok(error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevicePollError {
    AuthorizationPending,
    SlowDown,
    AuthorizationDeclined,
    Expired,
    InteractionRequired,
    Other,
}

fn classify_device_poll_error(value: &str) -> DevicePollError {
    match value {
        "authorization_pending" => DevicePollError::AuthorizationPending,
        "slow_down" => DevicePollError::SlowDown,
        "authorization_declined" => DevicePollError::AuthorizationDeclined,
        "expired_token" | "bad_verification_code" => DevicePollError::Expired,
        "interaction_required" => DevicePollError::InteractionRequired,
        _ => DevicePollError::Other,
    }
}

fn validate_verification_uri(value: &str, allow_http: bool) -> Result<()> {
    let valid = value.starts_with("https://")
        || (allow_http
            && (value.starts_with("http://127.0.0.1:") || value.starts_with("http://localhost:")));
    if !valid || value.len() > 2048 || value.contains('@') {
        return Err(provider_error(
            ErrorCode::AuthProviderUnavailable,
            "request_device_code",
            None,
            "device authorization verification URI violates transport policy",
        ));
    }
    Ok(())
}

fn checkpoint(operation: &OperationController) -> Result<()> {
    if operation.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

fn cancelled() -> GrapheneError {
    GrapheneError::new(
        ErrorCode::AuthCancelled,
        ErrorKind::Cancelled,
        "authentication operation was cancelled",
    )
}

fn map_transport(error: GrapheneError) -> GrapheneError {
    if error.is_cancelled() {
        return cancelled();
    }

    provider_error(
        ErrorCode::AuthProviderUnavailable,
        "network",
        None,
        "authentication provider transport failed",
    )
    .with_source(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_poll_errors_preserve_protocol_semantics() {
        assert_eq!(
            classify_device_poll_error("authorization_pending"),
            DevicePollError::AuthorizationPending
        );
        assert_eq!(
            classify_device_poll_error("slow_down"),
            DevicePollError::SlowDown
        );
        assert_eq!(
            classify_device_poll_error("authorization_declined"),
            DevicePollError::AuthorizationDeclined
        );
        assert_eq!(
            classify_device_poll_error("expired_token"),
            DevicePollError::Expired
        );
        assert_eq!(
            classify_device_poll_error("bad_verification_code"),
            DevicePollError::Expired
        );
        assert_eq!(
            classify_device_poll_error("interaction_required"),
            DevicePollError::InteractionRequired
        );
        assert_eq!(
            classify_device_poll_error("unknown"),
            DevicePollError::Other
        );
    }

    #[test]
    fn malformed_oauth_error_and_unsafe_verification_uri_are_rejected() {
        let malformed = BoundedResponse {
            status: 400,
            body: b"{not-json".to_vec(),
        };
        assert_eq!(
            parse_oauth_error(&malformed)
                .expect_err("malformed provider response must fail")
                .code,
            ErrorCode::AuthProviderUnavailable
        );
        assert!(validate_verification_uri("https://example.invalid/device", false).is_ok());
        assert!(validate_verification_uri("http://example.invalid/device", false).is_err());
        assert!(validate_verification_uri("http://127.0.0.1:1234/device", true).is_ok());
    }
}
