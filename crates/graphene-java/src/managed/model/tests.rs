use super::*;
use graphene_core::{ArtifactIntegrity, ArtifactKind, ArtifactSource, CachePolicy, Sha256Digest};

fn release(image: JavaImageKind) -> ManagedJavaRelease {
    let digest: Sha256Digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        .parse()
        .expect("fixture digest");
    let mut artifact = Artifact::new(
        vec![ArtifactSource::new(
            "https://example.invalid/runtime.tar.gz",
        )],
        ArtifactIntegrity::none().with_sha256(digest),
    );

    artifact.kind = ArtifactKind::Binary;
    artifact.cache_policy = CachePolicy::UseVerified;

    ManagedJavaRelease {
        runtime_id: ManagedRuntimeId::new(),
        provider: "fixture".into(),
        distribution: "fixture-jdk".into(),
        release_name: "fixture-21.0.1".into(),
        version: "21.0.1".into(),
        major_version: 21,
        vendor: JavaVendor::Other("Fixture Vendor".into()),
        os: JavaOperatingSystem::Linux,
        architecture: JavaArchitecture::X86_64,
        image,
        archive_format: ManagedArchiveFormat::TarGz,
        archive_sha256: digest,
        artifact,
    }
}

#[test]
fn request_rejects_unsupported_platform() {
    let request = ManagedJavaRequest {
        major_version: 21,
        os: JavaOperatingSystem::Other,
        architecture: JavaArchitecture::X86_64,
        preferred_image: JavaImageKind::Jre,
    };
    assert_eq!(
        request.validate().expect_err("unsupported OS").code,
        ErrorCode::PlatformUnsupported
    );
}

#[test]
fn jdk_can_satisfy_a_jre_request_but_not_the_reverse() {
    let jre_request = ManagedJavaRequest {
        major_version: 21,
        os: JavaOperatingSystem::Linux,
        architecture: JavaArchitecture::X86_64,
        preferred_image: JavaImageKind::Jre,
    };
    assert!(ManagedJavaInstallPlan::new(jre_request, release(JavaImageKind::Jdk)).is_ok());

    let jdk_request = ManagedJavaRequest {
        major_version: 21,
        os: JavaOperatingSystem::Linux,
        architecture: JavaArchitecture::X86_64,
        preferred_image: JavaImageKind::Jdk,
    };
    assert_eq!(
        ManagedJavaInstallPlan::new(jdk_request, release(JavaImageKind::Jre))
            .expect_err("JRE cannot satisfy JDK request")
            .code,
        ErrorCode::JavaManagedReleaseUnavailable
    );
}

#[test]
fn release_requires_matching_sha256_integrity() {
    let mut invalid = release(JavaImageKind::Jre);
    invalid.artifact.integrity = ArtifactIntegrity::none();
    assert_eq!(
        invalid.validate().expect_err("missing checksum").code,
        ErrorCode::JavaManagedReleaseUnavailable
    );
}

#[test]
fn runtime_rejects_unsafe_relative_executable_path() {
    let release = release(JavaImageKind::Jre);
    let runtime = ManagedJavaRuntime {
        schema_version: MANAGED_RUNTIME_SCHEMA_VERSION,
        id: release.runtime_id,
        provider: release.provider,
        distribution: release.distribution,
        release_name: release.release_name,
        version: release.version,
        major_version: release.major_version,
        vendor: release.vendor,
        os: release.os,
        architecture: release.architecture,
        image: release.image,
        archive_sha256: release.archive_sha256,
        executable_relative_path: "../bin/java".into(),
    };
    assert_eq!(
        runtime.validate().expect_err("traversal").code,
        ErrorCode::JavaManagedRuntimeCorrupt
    );
}
