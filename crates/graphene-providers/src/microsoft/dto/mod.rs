use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct DeviceCodeDto {
    pub(super) device_code: String,
    pub(super) user_code: String,
    pub(super) verification_uri: String,
    pub(super) expires_in: u64,
    pub(super) interval: Option<u64>,
    pub(super) message: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct TokenDto {
    pub(super) access_token: String,
    pub(super) refresh_token: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct OAuthErrorDto {
    pub(super) error: String,
}

#[derive(Deserialize)]
pub(super) struct XboxTokenDto {
    #[serde(rename = "Token")]
    pub(super) token: String,
    #[serde(rename = "DisplayClaims")]
    pub(super) display_claims: XboxClaimsDto,
}

#[derive(Deserialize)]
pub(super) struct XboxClaimsDto {
    pub(super) xui: Vec<XboxUserClaimDto>,
}

#[derive(Deserialize)]
pub(super) struct XboxUserClaimDto {
    pub(super) uhs: String,
    #[serde(default)]
    pub(super) xid: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct MinecraftTokenDto {
    pub(super) access_token: String,
}

#[derive(Deserialize)]
pub(super) struct EntitlementsDto {
    #[serde(default)]
    pub(super) items: Vec<EntitlementItemDto>,
}

#[derive(Deserialize)]
pub(super) struct EntitlementItemDto {
    #[serde(default)]
    pub(super) name: String,
}

#[derive(Deserialize)]
pub(super) struct MinecraftProfileDto {
    pub(super) id: String,
    pub(super) name: String,
}
