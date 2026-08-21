use super::*;

pub(super) trait CoordinateKey {
    fn to_string_key(&self) -> String;
}

impl CoordinateKey for MavenCoordinate {
    fn to_string_key(&self) -> String {
        let classifier = self
            .classifier
            .as_deref()
            .map(|v| format!(":{v}"))
            .unwrap_or_default();
        let extension = if self.extension == "jar" {
            String::new()
        } else {
            format!("@{}", self.extension)
        };

        format!(
            "{}:{}:{}{}{}",
            self.group, self.artifact, self.version, classifier, extension
        )
    }
}

pub(super) fn normalize_remote_library(
    library: &LibraryDto,
    default_maven_base: &str,
    allow_http: bool,
) -> Result<(MavenCoordinate, ResolvedArtifact)> {
    let coordinate = MavenCoordinate::parse(&library.name).map_err(|source| {
        loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader profile contains an invalid Maven coordinate",
        )
        .with_source(source)
    })?;
    let repository_path = coordinate.repository_path()?;
    let download = library
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref());
    let resolved = if let Some(download) = download {
        remote_download(
            download,
            &repository_path,
            library.url.as_deref().unwrap_or(default_maven_base),
            allow_http,
        )?
    } else {
        return Err(loader_error(
            ErrorCode::LoaderArtifactUnverifiable,
            "loader processor dependency lacks verified download metadata",
        ));
    };

    Ok((coordinate, resolved))
}

fn remote_download(
    download: &DownloadDto,
    repository_path: &ManagedPath,
    repository_base: &str,
    allow_http: bool,
) -> Result<ResolvedArtifact> {
    let relative = match &download.path {
        Some(path) => {
            let path = ManagedPath::new(path.clone()).map_err(|source| {
                loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "loader artifact path is unsafe",
                )
                .with_source(source)
            })?;

            if path != *repository_path {
                return Err(loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "loader artifact path does not match its Maven coordinate",
                ));
            }
            path
        }

        None => repository_path.clone(),
    };

    let url = download
        .url
        .clone()
        .unwrap_or_else(|| join_base(repository_base, relative.as_str()));
    let mut integrity = ArtifactIntegrity::none();

    if let Some(value) = &download.sha1 {
        integrity = integrity.with_sha1(value.parse().map_err(|source| {
            loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader artifact SHA-1 is invalid",
            )
            .with_source(source)
        })?);
    }

    if let Some(value) = &download.sha256 {
        integrity = integrity.with_sha256(value.parse().map_err(|source| {
            loader_error(
                ErrorCode::LoaderProfileInvalid,
                "loader artifact SHA-256 is invalid",
            )
            .with_source(source)
        })?);
    }

    Ok(ResolvedArtifact {
        artifact: artifact(url, integrity, download.size, allow_http)?,
        relative_path: ManagedPath::new(format!("shared/libraries/{}", relative.as_str()))?,
    })
}

pub(super) fn normalize_version_libraries(
    values: Vec<LibraryDto>,
    default_maven_base: &str,
    allow_http: bool,
    generated: &BTreeMap<String, GeneratedOutput>,
) -> Result<Vec<Library>> {
    if values.len() > MAX_PROFILE_LIBRARIES {
        return Err(loader_error(
            ErrorCode::LoaderProfileInvalid,
            "loader version metadata contains too many libraries",
        ));
    }

    values
        .into_iter()
        .map(|library| {
            let coordinate = MavenCoordinate::parse(&library.name).map_err(|source| {
                loader_error(
                    ErrorCode::LoaderProfileInvalid,
                    "loader version metadata contains an invalid Maven coordinate",
                )
                    .with_source(source)
            })?;

            let rules = library
                .rules
                .iter()
                .map(normalize_rule)
                .collect::<Result<Vec<_>>>()?;

            let artifact = if let Some(download) = library
                .downloads
                .as_ref()
                .and_then(|downloads| downloads.artifact.as_ref())
            {
                Some(remote_download(
                    download,
                    &coordinate.repository_path()?,
                    library.url.as_deref().unwrap_or(default_maven_base),
                    allow_http,
                )?)
            } else if let Some(output) = generated.get(&coordinate.to_string_key()) {
                Some(generated_resolved_artifact(output)?)
            } else {
                return Err(loader_error(
                    ErrorCode::LoaderArtifactUnverifiable,
                    "loader runtime library is neither a verified remote artifact nor a declared generated output",
                ));
            };

            Ok(Library {
                coordinate,
                rules,
                artifact,
                classifiers: BTreeMap::new(),
                natives: BTreeMap::new(),
            })
        })
        .collect()
}

fn generated_resolved_artifact(output: &GeneratedOutput) -> Result<ResolvedArtifact> {
    if !output.expected_integrity.is_verifiable() {
        return Err(loader_error(
            ErrorCode::LoaderArtifactUnverifiable,
            "generated loader runtime artifact has no declared digest",
        ));
    }

    let mut id = [0_u8; 16];

    if let Some(sha256) = output.expected_integrity.sha256() {
        id.copy_from_slice(&sha256.as_bytes()[..16]);
    } else if let Some(sha1) = output.expected_integrity.sha1() {
        id.copy_from_slice(&sha1.as_bytes()[..16]);
    }

    Ok(ResolvedArtifact {
        artifact: Artifact {
            id: ArtifactId::from_bytes(id),
            kind: ArtifactKind::Binary,
            sources: Vec::new(),
            integrity: output.expected_integrity.clone(),
            expected_size: output.expected_size,
            cache_policy: graphene_core::CachePolicy::UseVerified,
        },
        relative_path: output.managed_destination.clone(),
    })
}
