use super::*;
use crate::{
    id::{ContentFileRef, ContentProviderId, ContentVersionRef},
    model::{
        compatibility::{CompatibilityResult, InstanceContentContext, ReleaseChannelPolicy},
        file::{ContentFile, FileRole},
        release::ReleaseChannel,
        version::{ContentVersion, EnvironmentSupport},
    },
};
use graphene_core::{ArtifactIntegrity, ArtifactSource, InstanceId};
use graphene_minecraft::LoaderKind;

fn make_test_version(
    game_versions: Vec<&str>,
    loaders: Vec<LoaderKind>,
    channel: ReleaseChannel,
    env: EnvironmentSupport,
) -> ContentVersion {
    let provider = ContentProviderId::new("modrinth").unwrap();
    let version_ref = ContentVersionRef::new(provider.clone(), "test", "1.0").unwrap();
    let file_ref = ContentFileRef::new(provider, "test", "1.0", "test.jar").unwrap();
    let integrity = ArtifactIntegrity::none()
        .with_sha1("2fd4e1c67a2d28fced849ee1bb76e7391b93eb12".parse().unwrap());
    let sources = vec![ArtifactSource::new("https://example.com/test.jar")];
    let file = ContentFile {
        file_ref,
        filename: "test.jar".to_string(),
        role: FileRole::Primary,
        size: 1000,
        integrity,
        sources,
        available: true,
        murmur2_fingerprint: None,
        md5: None,
    };

    ContentVersion {
        version_ref,
        version_number: "1.0".to_string(),
        display_name: "Test Mod 1.0".to_string(),
        release_channel: channel,
        game_versions: game_versions.into_iter().map(String::from).collect(),
        loaders,
        environment: env,
        dependencies: Vec::new(),
        files: vec![file],
        date_published: None,
        downloads: Some(10),
        available: true,
    }
}

#[test]
fn exact_minecraft_and_loader_compatibility() {
    let context =
        InstanceContentContext::new(InstanceId::new(), "1.21.1", Some(LoaderKind::Fabric), None);

    let v1 = make_test_version(
        vec!["1.21.1"],
        vec![LoaderKind::Fabric],
        ReleaseChannel::Release,
        EnvironmentSupport::Both,
    );
    assert_eq!(
        evaluate_compatibility(&v1, &context, ReleaseChannelPolicy::ReleaseOnly),
        CompatibilityResult::Compatible
    );

    let v_mc_mismatch = make_test_version(
        vec!["1.20.1"],
        vec![LoaderKind::Fabric],
        ReleaseChannel::Release,
        EnvironmentSupport::Both,
    );
    assert!(
        !evaluate_compatibility(&v_mc_mismatch, &context, ReleaseChannelPolicy::ReleaseOnly)
            .is_compatible()
    );

    let v_loader_mismatch = make_test_version(
        vec!["1.21.1"],
        vec![LoaderKind::Forge],
        ReleaseChannel::Release,
        EnvironmentSupport::Both,
    );
    assert!(
        !evaluate_compatibility(
            &v_loader_mismatch,
            &context,
            ReleaseChannelPolicy::ReleaseOnly
        )
        .is_compatible()
    );

    let v_server_only = make_test_version(
        vec!["1.21.1"],
        vec![LoaderKind::Fabric],
        ReleaseChannel::Release,
        EnvironmentSupport::ServerOnly,
    );
    assert!(
        !evaluate_compatibility(&v_server_only, &context, ReleaseChannelPolicy::ReleaseOnly)
            .is_compatible()
    );
}

#[test]
fn release_channel_policy_enforcement() {
    let context =
        InstanceContentContext::new(InstanceId::new(), "1.21.1", Some(LoaderKind::Fabric), None);

    let v_beta = make_test_version(
        vec!["1.21.1"],
        vec![LoaderKind::Fabric],
        ReleaseChannel::Beta,
        EnvironmentSupport::Both,
    );

    assert!(
        !evaluate_compatibility(&v_beta, &context, ReleaseChannelPolicy::ReleaseOnly)
            .is_compatible()
    );
    assert!(
        evaluate_compatibility(&v_beta, &context, ReleaseChannelPolicy::ReleaseOrBeta)
            .is_compatible()
    );
    assert!(evaluate_compatibility(&v_beta, &context, ReleaseChannelPolicy::Any).is_compatible());
}
