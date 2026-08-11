use super::{config::AdoptiumProviderConfig, dto::ReleaseDto};
use graphene_core::{
    Artifact, ArtifactId, ArtifactIntegrity, ArtifactKind, ArtifactSource, ErrorCode, ErrorKind,
    GrapheneError, ManagedRuntimeId, OperationController, Result, Sha256Digest,
};
use graphene_java::{
    JavaArchitecture, JavaDistributionCapabilities, JavaDistributionFuture,
    JavaDistributionProvider, JavaImageKind, JavaOperatingSystem, JavaVendor, ManagedArchiveFormat,
    ManagedJavaRelease, ManagedJavaRequest,
};
use graphene_network::NetworkClient;

const MAX_RELEASE_METADATA_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AdoptiumProvider {
    network: NetworkClient,
    config: AdoptiumProviderConfig,
}

impl std::fmt::Debug for AdoptiumProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdoptiumProvider")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AdoptiumProvider {
    pub fn new(network: NetworkClient, config: AdoptiumProviderConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self { network, config })
    }

    async fn resolve(
        &self,
        request: &ManagedJavaRequest,
        operation: &OperationController,
    ) -> Result<ManagedJavaRelease> {
        request.validate()?;
        if !self.capabilities().supports(request) {
            return Err(java_error(
                ErrorCode::JavaManagedReleaseUnavailable,
                "Adoptium cannot satisfy the requested platform/image",
            ));
        }

        let architecture = map_arch(request.architecture)?;
        let os = map_os(request.os)?;
        let preferred = match request.preferred_image {
            JavaImageKind::Jre => "jre",
            JavaImageKind::Jdk => "jdk",
            _ => {
                return Err(java_error(
                    ErrorCode::JavaManagedReleaseUnavailable,
                    "managed Java image type is unsupported by Adoptium",
                ));
            }
        };

        if let Some(release) = self
            .resolve_image(request, architecture, os, preferred, operation)
            .await?
        {
            return Ok(release);
        }

        if request.preferred_image == JavaImageKind::Jre
            && let Some(release) = self
                .resolve_image(request, architecture, os, "jdk", operation)
                .await?
        {
            return Ok(release);
        }

        Err(java_error(
            ErrorCode::JavaManagedReleaseUnavailable,
            "managed Java provider returned no compatible release",
        ))
    }

    async fn resolve_image(
        &self,
        request: &ManagedJavaRequest,
        architecture: &str,
        os: &str,
        image: &str,
        operation: &OperationController,
    ) -> Result<Option<ManagedJavaRelease>> {
        let url = self
            .config
            .release_url(request.major_version, architecture, image, os);
        let response = self
            .network
            .get_response_bounded(
                &url,
                MAX_RELEASE_METADATA_BYTES,
                self.config.allow_http(),
                operation,
            )
            .await
            .map_err(|error| {
                java_error(
                    ErrorCode::JavaManagedProviderUnavailable,
                    "managed Java release metadata request failed",
                )
                .with_source(error)
            })?;

        if response.status == 404 {
            return Ok(None);
        }

        if !(200..300).contains(&response.status) {
            return Err(java_error(
                ErrorCode::JavaManagedProviderUnavailable,
                "managed Java provider returned a non-success status",
            )
            .with_context("status", response.status.to_string()));
        }

        let mut releases: Vec<ReleaseDto> =
            serde_json::from_slice(&response.body).map_err(|source| {
                java_error(
                    ErrorCode::JavaManagedProviderUnavailable,
                    "managed Java provider returned malformed JSON",
                )
                .with_source(source)
            })?;
        releases.retain(|release| {
            release.version.major == request.major_version && release.binary.image_type == image
        });
        releases.sort_by(|left, right| {
            version_key(&right.version.semver)
                .cmp(&version_key(&left.version.semver))
                .then_with(|| left.release_name.cmp(&right.release_name))
        });
        releases
            .into_iter()
            .next()
            .map(|release| normalize_release(release, request, self.config.allow_http()))
            .transpose()
    }
}

impl JavaDistributionProvider for AdoptiumProvider {
    fn provider_id(&self) -> &'static str {
        "adoptium"
    }

    fn capabilities(&self) -> JavaDistributionCapabilities {
        JavaDistributionCapabilities {
            sha256: true,
            jre: true,
            jdk: true,
            windows: true,
            linux: true,
            macos: true,
            x86: true,
            x86_64: true,
            aarch64: true,
        }
    }

    fn resolve_release<'a>(
        &'a self,
        request: &'a ManagedJavaRequest,
        operation: &'a OperationController,
    ) -> JavaDistributionFuture<'a> {
        Box::pin(self.resolve(request, operation))
    }
}

fn normalize_release(
    dto: ReleaseDto,
    request: &ManagedJavaRequest,
    allow_http: bool,
) -> Result<ManagedJavaRelease> {
    if dto.release_name.is_empty()
        || dto.release_name.len() > 160
        || dto.version.semver.is_empty()
        || dto.version.semver.len() > 64
        || dto.vendor.len() > 64
        || dto.binary.package.name.len() > 512
        || dto.binary.package.link.len() > 2048
    {
        return Err(java_error(
            ErrorCode::JavaManagedReleaseUnavailable,
            "managed Java provider release fields are invalid",
        ));
    }

    let architecture = JavaArchitecture::normalize(&dto.binary.architecture);
    let os = match dto.binary.os.as_str() {
        "windows" => JavaOperatingSystem::Windows,
        "linux" => JavaOperatingSystem::Linux,
        "mac" | "macos" => JavaOperatingSystem::MacOS,
        _ => JavaOperatingSystem::Other,
    };
    let image = match dto.binary.image_type.as_str() {
        "jre" => JavaImageKind::Jre,
        "jdk" => JavaImageKind::Jdk,
        _ => {
            return Err(java_error(
                ErrorCode::JavaManagedReleaseUnavailable,
                "managed Java provider returned unsupported image type",
            ));
        }
    };
    let image_allowed = image == request.preferred_image
        || (request.preferred_image == JavaImageKind::Jre && image == JavaImageKind::Jdk);

    if architecture != request.architecture || os != request.os || !image_allowed {
        return Err(java_error(
            ErrorCode::JavaManagedReleaseUnavailable,
            "managed Java provider returned mismatched release metadata",
        ));
    }

    let sha256: Sha256Digest = dto.binary.package.checksum.parse().map_err(|source| {
        java_error(
            ErrorCode::JavaManagedReleaseUnavailable,
            "managed Java release checksum is missing or invalid",
        )
        .with_source(source)
    })?;

    let link = dto.binary.package.link;
    let valid_link = link.starts_with("https://")
        || (allow_http
            && (link.starts_with("http://127.0.0.1:") || link.starts_with("http://localhost:")));

    if !valid_link || link.contains('@') {
        return Err(java_error(
            ErrorCode::JavaManagedReleaseUnavailable,
            "managed Java archive URL violates transport policy",
        ));
    }

    let archive_format = archive_format(&dto.binary.package.name, &link)?;
    let mut artifact = Artifact::new(
        vec![ArtifactSource::new(link).with_label("adoptium")],
        ArtifactIntegrity::none().with_sha256(sha256),
    );

    artifact.id = ArtifactId::from_bytes(first_16(sha256.as_bytes()));
    artifact.kind = ArtifactKind::Binary;

    if let Some(size) = dto.binary.package.size {
        if size == 0 {
            return Err(java_error(
                ErrorCode::JavaManagedReleaseUnavailable,
                "managed Java provider returned an invalid archive size",
            ));
        }
        artifact = artifact.with_expected_size(size);
    }

    let runtime_id = ManagedRuntimeId::from_bytes(first_16(sha256.as_bytes()));
    let release = ManagedJavaRelease {
        runtime_id,
        provider: "adoptium".into(),
        distribution: "eclipse-temurin".into(),
        release_name: dto.release_name,
        version: dto.version.semver,
        major_version: dto.version.major,
        vendor: if dto.vendor.to_ascii_lowercase().contains("eclipse")
            || dto.vendor.to_ascii_lowercase().contains("adoptium")
        {
            JavaVendor::Adoptium
        } else {
            JavaVendor::Other(dto.vendor)
        },
        os,
        architecture,
        image,
        archive_format,
        archive_sha256: sha256,
        artifact,
    };
    release.validate()?;

    Ok(release)
}

fn map_arch(value: JavaArchitecture) -> Result<&'static str> {
    match value {
        JavaArchitecture::X86 => Ok("x86"),
        JavaArchitecture::X86_64 => Ok("x64"),
        JavaArchitecture::AArch64 => Ok("aarch64"),
        JavaArchitecture::Other => Err(java_error(
            ErrorCode::PlatformUnsupported,
            "unsupported Java architecture",
        )),
        _ => Err(java_error(
            ErrorCode::PlatformUnsupported,
            "unknown Java architecture is unsupported",
        )),
    }
}

fn map_os(value: JavaOperatingSystem) -> Result<&'static str> {
    match value {
        JavaOperatingSystem::Windows => Ok("windows"),
        JavaOperatingSystem::Linux => Ok("linux"),
        JavaOperatingSystem::MacOS => Ok("mac"),
        JavaOperatingSystem::Other => Err(java_error(
            ErrorCode::PlatformUnsupported,
            "unsupported Java operating system",
        )),
        _ => Err(java_error(
            ErrorCode::PlatformUnsupported,
            "unknown Java operating system is unsupported",
        )),
    }
}

fn archive_format(name: &str, link: &str) -> Result<ManagedArchiveFormat> {
    let value = if name.is_empty() { link } else { name };
    if value.ends_with(".zip") {
        Ok(ManagedArchiveFormat::Zip)
    } else if value.ends_with(".tar.gz") || value.ends_with(".tgz") {
        Ok(ManagedArchiveFormat::TarGz)
    } else {
        Err(java_error(
            ErrorCode::JavaManagedReleaseUnavailable,
            "managed Java provider returned unsupported archive format",
        ))
    }
}

fn first_16(bytes: &[u8; 32]) -> [u8; 16] {
    let mut out = [0_u8; 16];
    out.copy_from_slice(&bytes[..16]);
    out
}

fn version_key(value: &str) -> Vec<u32> {
    value
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn java_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Java, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> ManagedJavaRequest {
        ManagedJavaRequest {
            major_version: 21,
            os: JavaOperatingSystem::Linux,
            architecture: JavaArchitecture::X86_64,
            preferred_image: JavaImageKind::Jre,
        }
    }

    fn release(checksum: &str, architecture: &str, link: &str) -> ReleaseDto {
        serde_json::from_value(json!({
            "binary": {
                "architecture": architecture,
                "image_type": "jre",
                "os": "linux",
                "package": {
                    "checksum": checksum,
                    "link": link,
                    "name": "OpenJDK21U-jre-fixture.tar.gz",
                    "size": 1234
                }
            },
            "release_name": "jdk-21.0.4+7",
            "vendor": "Eclipse Adoptium",
            "version": { "major": 21, "semver": "21.0.4+7" }
        }))
        .expect("fixture provider DTO")
    }

    #[test]
    fn release_normalization_requires_sha256_and_matching_platform() {
        let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let normalized = normalize_release(
            release(digest, "x64", "https://example.invalid/runtime.tar.gz"),
            &request(),
            false,
        )
        .expect("valid release");
        assert_eq!(normalized.major_version, 21);
        assert_eq!(normalized.architecture, JavaArchitecture::X86_64);
        assert_eq!(normalized.archive_format, ManagedArchiveFormat::TarGz);

        let missing = normalize_release(
            release("", "x64", "https://example.invalid/runtime.tar.gz"),
            &request(),
            false,
        )
        .expect_err("missing checksum must fail");
        assert_eq!(missing.code, ErrorCode::JavaManagedReleaseUnavailable);

        let mismatch = normalize_release(
            release(digest, "aarch64", "https://example.invalid/runtime.tar.gz"),
            &request(),
            false,
        )
        .expect_err("architecture mismatch must fail");
        assert_eq!(mismatch.code, ErrorCode::JavaManagedReleaseUnavailable);
    }

    #[test]
    fn production_release_normalization_rejects_plain_http() {
        let digest = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let error = normalize_release(
            release(digest, "x64", "http://example.invalid/runtime.tar.gz"),
            &request(),
            false,
        )
        .expect_err("production HTTP must fail");
        assert_eq!(error.code, ErrorCode::JavaManagedReleaseUnavailable);

        assert!(
            normalize_release(
                release(digest, "x64", "http://127.0.0.1:1234/runtime.tar.gz"),
                &request(),
                true,
            )
            .is_ok()
        );
    }
}
