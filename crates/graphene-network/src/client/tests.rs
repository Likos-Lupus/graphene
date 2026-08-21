use super::download::{classify_status, jittered_delay, ordered_sources};
use graphene_core::{Artifact, ArtifactId, ArtifactIntegrity, ArtifactSource, ErrorCode};
use reqwest::StatusCode;
use std::time::Duration;

#[test]
fn source_order_is_priority_then_declaration_order() {
    let artifact = Artifact::new(
        vec![
            ArtifactSource::new("https://one.invalid").with_priority(5),
            ArtifactSource::new("https://two.invalid").with_priority(1),
            ArtifactSource::new("https://three.invalid").with_priority(1),
        ],
        ArtifactIntegrity::none(),
    );
    let ordered = ordered_sources(&artifact)
        .into_iter()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(ordered, vec![1, 2, 0]);
}

#[test]
fn status_mapping_distinguishes_retryability() {
    let transient = classify_status(StatusCode::SERVICE_UNAVAILABLE);
    assert!(transient.retryable);
    assert_eq!(transient.error.code, ErrorCode::NetworkStatusError);

    let permanent = classify_status(StatusCode::NOT_FOUND);
    assert!(!permanent.retryable);
    assert_eq!(permanent.error.code, ErrorCode::NetworkStatusError);
}

#[test]
fn jitter_stays_positive_and_bounded() {
    let id = ArtifactId::new();
    let base = Duration::from_millis(100);
    let maximum = Duration::from_millis(110);

    for attempt in 2..=10 {
        let delay = jittered_delay(base, maximum, id, attempt);
        assert!(!delay.is_zero());
        assert!(delay <= maximum);
    }
}
