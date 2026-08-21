use crate::{NetworkConfig, ProxyPolicy};
use futures_util::StreamExt;
use graphene_core::{
    ErrorCode, ErrorKind, GrapheneError, OperationController, Result, SensitiveString, Sha1Digest,
    Sha256Digest,
};
use reqwest::Client;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

mod download;
mod verification;

#[cfg(test)]
mod tests;

/// Completed streamed transfer data. The body itself remains disk-backed at the caller-provided
/// temporary path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferResult {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha1: Sha1Digest,
    pub sha256: Sha256Digest,
    pub source_host: Option<String>,
}

/// Verification result for an existing disk-backed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFile {
    pub bytes: u64,
    pub sha1: Sha1Digest,
    pub sha256: Sha256Digest,
}

/// Bounded in-memory HTTP response used by protocol adapters without exposing Reqwest types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Shared provider-neutral HTTP client with bounded concurrency.
#[derive(Clone)]
pub struct NetworkClient {
    client: Client,
    protocol_client: Client,
    config: Arc<NetworkConfig>,
    downloads: Arc<Semaphore>,
}

impl std::fmt::Debug for NetworkClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}
fn build_http_client(config: &NetworkConfig, no_redirects: bool) -> Result<Client> {
    let max_redirects = config.redirect_policy.max_redirects;
    let redirect_policy = if no_redirects {
        reqwest::redirect::Policy::none()
    } else {
        reqwest::redirect::Policy::custom(move |attempt| {
            let next = attempt.url();
            if attempt.previous().len() >= max_redirects {
                return attempt.error("redirect limit exceeded");
            }

            if !matches!(next.scheme(), "http" | "https")
                || !next.username().is_empty()
                || next.password().is_some()
            {
                return attempt.error("redirect target rejected");
            }

            if attempt
                .previous()
                .last()
                .is_some_and(|previous| previous.scheme() == "https")
                && next.scheme() != "https"
            {
                return attempt.error("HTTPS redirect downgrade rejected");
            }

            attempt.follow()
        })
    };

    let mut builder = Client::builder()
        .connect_timeout(config.connect_timeout)
        .timeout(config.request_timeout)
        .redirect(redirect_policy)
        .user_agent(config.user_agent.clone());

    builder = match &config.proxy {
        ProxyPolicy::System => builder,
        ProxyPolicy::None => builder.no_proxy(),
        ProxyPolicy::Explicit(_) => {
            let proxy = reqwest::Proxy::all(
                config
                    .proxy
                    .explicit_url()
                    .expect("explicit proxy variant has URL"),
            )
            .map_err(|_source| {
                GrapheneError::new(
                    ErrorCode::NetworkProxyInvalid,
                    ErrorKind::Configuration,
                    "explicit proxy configuration is invalid",
                )
            })?;
            builder.proxy(proxy)
        }
    };

    builder.build().map_err(|source| {
        GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            "failed to construct shared HTTP client",
        )
        .with_source(source)
    })
}

impl NetworkClient {
    /// Builds the shared HTTP client. TLS certificate validation remains enabled; there is no
    /// ordinary configuration switch to disable it.
    pub fn new(config: NetworkConfig) -> Result<Self> {
        config.validate()?;
        let client = build_http_client(&config, false)?;
        // Credential-bearing/provider-protocol requests never follow redirects. This prevents a
        // 307/308 response from forwarding form bodies or bearer credentials to another origin.
        let protocol_client = build_http_client(&config, true)?;
        Ok(Self {
            downloads: Arc::new(Semaphore::new(config.max_concurrent_downloads)),
            client,
            protocol_client,
            config: Arc::new(config),
        })
    }

    /// Returns immutable Graphene-owned network policy.
    #[must_use]
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

impl NetworkClient {
    /// Performs a bounded metadata GET through the same configured HTTP client.
    ///
    /// The response is kept in memory only up to `max_bytes`. URLs remain transport inputs and are
    /// never copied into Graphene errors. Plain HTTP must be opted into explicitly for local fixture
    /// servers; HTTPS requests reject redirect downgrade.
    pub async fn get_bytes_bounded(
        &self,
        raw_url: &str,
        max_bytes: usize,
        allow_http: bool,
        operation: &OperationController,
    ) -> Result<Vec<u8>> {
        if max_bytes == 0 {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "metadata response bound must be non-zero",
            ));
        }

        checkpoint(operation, ErrorCode::DownloadCancelled)?;
        let _permit = self.acquire_download_permit(operation).await?;
        let url = reqwest::Url::parse(raw_url).map_err(|source| {
            GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "metadata URL is invalid",
            )
            .with_source(source)
        })?;

        validate_metadata_url(&url, allow_http)?;
        let original_scheme = url.scheme().to_owned();
        let token = operation.cancellation_token();
        let response = tokio::select! {
            () = token.cancelled() => return Err(cancelled_error(ErrorCode::DownloadCancelled)),
            response = self.client.get(url).send() => response.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::NetworkRequestFailed,
                    ErrorKind::Network,
                    "metadata request failed",
                ).with_source(source)
            })?,
        };

        checkpoint(operation, ErrorCode::DownloadCancelled)?;
        if original_scheme == "https" && response.url().scheme() != "https" {
            return Err(GrapheneError::new(
                ErrorCode::NetworkRedirectRejected,
                ErrorKind::Network,
                "metadata redirect attempted to downgrade HTTPS",
            ));
        }

        validate_metadata_url(response.url(), allow_http)?;
        if !response.status().is_success() {
            return Err(GrapheneError::new(
                ErrorCode::NetworkStatusError,
                ErrorKind::Network,
                "metadata endpoint returned a non-success status",
            )
            .with_context("status", response.status().as_u16().to_string()));
        }

        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(GrapheneError::new(
                ErrorCode::DownloadSizeMismatch,
                ErrorKind::Network,
                "metadata response exceeds configured size limit",
            )
            .with_context("max_bytes", max_bytes.to_string()));
        }

        let mut bytes = Vec::with_capacity(
            response.content_length().unwrap_or(0).min(max_bytes as u64) as usize,
        );
        let mut stream = response.bytes_stream();

        while let Some(chunk) = tokio::select! {
            () = token.cancelled() => return Err(cancelled_error(ErrorCode::DownloadCancelled)),
            next = stream.next() => next,
        } {
            let chunk = chunk.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::NetworkRequestFailed,
                    ErrorKind::Network,
                    "failed while reading metadata response",
                )
                .with_source(source)
            })?;

            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                return Err(GrapheneError::new(
                    ErrorCode::DownloadSizeMismatch,
                    ErrorKind::Network,
                    "metadata response exceeds configured size limit",
                )
                .with_context("max_bytes", max_bytes.to_string()));
            }
            bytes.extend_from_slice(&chunk);
        }

        Ok(bytes)
    }

    /// Performs a bounded credential-bearing form POST without automatic retries.
    pub async fn post_form_bounded(
        &self,
        raw_url: &str,
        fields: &[(&str, &str)],
        max_bytes: usize,
        allow_http: bool,
        operation: &OperationController,
    ) -> Result<BoundedResponse> {
        let body = encode_form(fields);
        self.send_bounded(
            raw_url,
            self.protocol_client
                .post(raw_url)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(body),
            max_bytes,
            allow_http,
            operation,
        )
        .await
    }

    /// Performs a bounded JSON POST without automatic retries.
    pub async fn post_json_bounded(
        &self,
        raw_url: &str,
        body: &[u8],
        max_bytes: usize,
        allow_http: bool,
        operation: &OperationController,
    ) -> Result<BoundedResponse> {
        self.send_bounded(
            raw_url,
            self.protocol_client
                .post(raw_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_vec()),
            max_bytes,
            allow_http,
            operation,
        )
        .await
    }

    /// Performs a bounded bearer-authenticated GET without exposing the token to formatting.
    pub async fn get_bearer_bounded(
        &self,
        raw_url: &str,
        bearer: &SensitiveString,
        max_bytes: usize,
        allow_http: bool,
        operation: &OperationController,
    ) -> Result<BoundedResponse> {
        self.send_bounded(
            raw_url,
            self.protocol_client
                .get(raw_url)
                .bearer_auth(bearer.expose_secret()),
            max_bytes,
            allow_http,
            operation,
        )
        .await
    }

    /// Performs a bounded status-preserving GET for provider metadata.
    pub async fn get_response_bounded(
        &self,
        raw_url: &str,
        max_bytes: usize,
        allow_http: bool,
        operation: &OperationController,
    ) -> Result<BoundedResponse> {
        self.send_bounded(
            raw_url,
            self.protocol_client.get(raw_url),
            max_bytes,
            allow_http,
            operation,
        )
        .await
    }

    async fn send_bounded(
        &self,
        raw_url: &str,
        request: reqwest::RequestBuilder,
        max_bytes: usize,
        allow_http: bool,
        operation: &OperationController,
    ) -> Result<BoundedResponse> {
        if max_bytes == 0 {
            return Err(GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "protocol response bound must be non-zero",
            ));
        }

        checkpoint(operation, ErrorCode::DownloadCancelled)?;
        let _permit = self.acquire_download_permit(operation).await?;
        let url = reqwest::Url::parse(raw_url).map_err(|source| {
            GrapheneError::new(
                ErrorCode::ConfigInvalid,
                ErrorKind::Configuration,
                "protocol URL is invalid",
            )
            .with_source(source)
        })?;

        validate_metadata_url(&url, allow_http)?;
        let original_scheme = url.scheme().to_owned();
        let token = operation.cancellation_token();
        let response = tokio::select! {
            () = token.cancelled() => return Err(cancelled_error(ErrorCode::DownloadCancelled)),
            response = request.send() => response.map_err(|source| {
                GrapheneError::new(ErrorCode::NetworkRequestFailed, ErrorKind::Network, "protocol request failed").with_source(source)
            })?,
        };

        checkpoint(operation, ErrorCode::DownloadCancelled)?;
        if original_scheme == "https" && response.url().scheme() != "https" {
            return Err(GrapheneError::new(
                ErrorCode::NetworkRedirectRejected,
                ErrorKind::Network,
                "protocol redirect attempted to downgrade HTTPS",
            ));
        }

        validate_metadata_url(response.url(), allow_http)?;
        let status = response.status().as_u16();
        if response
            .content_length()
            .is_some_and(|length| length > max_bytes as u64)
        {
            return Err(GrapheneError::new(
                ErrorCode::DownloadSizeMismatch,
                ErrorKind::Network,
                "protocol response exceeds configured size limit",
            )
            .with_context("max_bytes", max_bytes.to_string()));
        }

        let mut body = Vec::with_capacity(
            response.content_length().unwrap_or(0).min(max_bytes as u64) as usize,
        );
        let mut stream = response.bytes_stream();
        while let Some(chunk) = tokio::select! {
            () = token.cancelled() => return Err(cancelled_error(ErrorCode::DownloadCancelled)),
            next = stream.next() => next,
        } {
            let chunk = chunk.map_err(|source| {
                GrapheneError::new(
                    ErrorCode::NetworkRequestFailed,
                    ErrorKind::Network,
                    "failed while reading protocol response",
                )
                .with_source(source)
            })?;
            if body.len().saturating_add(chunk.len()) > max_bytes {
                return Err(GrapheneError::new(
                    ErrorCode::DownloadSizeMismatch,
                    ErrorKind::Network,
                    "protocol response exceeds configured size limit",
                )
                .with_context("max_bytes", max_bytes.to_string()));
            }

            body.extend_from_slice(&chunk);
        }

        Ok(BoundedResponse { status, body })
    }
}

fn validate_metadata_url(url: &reqwest::Url, allow_http: bool) -> Result<()> {
    let safe_scheme = url.scheme() == "https" || (allow_http && url.scheme() == "http");
    if !safe_scheme {
        return Err(GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            "metadata URL uses a disallowed scheme",
        ));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            "metadata URL must not contain userinfo",
        ));
    }

    if url.host_str().is_none() {
        return Err(GrapheneError::new(
            ErrorCode::ConfigInvalid,
            ErrorKind::Configuration,
            "metadata URL must have a host",
        ));
    }

    Ok(())
}

fn checkpoint(operation: &OperationController, code: ErrorCode) -> Result<()> {
    if operation.is_cancelled() {
        Err(cancelled_error(code))
    } else {
        Ok(())
    }
}

fn encode_form(fields: &[(&str, &str)]) -> String {
    let mut encoded = String::new();
    fields.iter().enumerate().for_each(|(index, (key, value))| {
        if index != 0 {
            encoded.push('&');
        }
        encode_form_component(key, &mut encoded);
        encoded.push('=');
        encode_form_component(value, &mut encoded);
    });
    encoded
}

fn encode_form_component(value: &str, output: &mut String) {
    use std::fmt::Write as _;
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(char::from(byte));
            }
            b' ' => output.push('+'),
            _ => {
                let _ = write!(output, "%{byte:02X}");
            }
        }
    }
}

pub(super) fn cancelled_error(code: ErrorCode) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Cancelled, "download was cancelled")
}
