use crate::{
    error::{cancelled_error, install_error},
    path::minecraft_path_to_platform,
};
use graphene_core::{ArtifactIntegrity, CancellationToken, ErrorCode, Result};
use graphene_minecraft::{GeneratedOutput, GeneratedOutputScope, ResolvedComponent};
use graphene_platform::{ManagedRelativePath, replace_file_safely};
use graphene_storage::DataRoot;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const MAX_PROVENANCE_BYTES: u64 = 64 * 1024;
pub(crate) const MAX_GENERATED_OUTPUT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct GeneratedProvenance {
    schema_version: u32,
    component_uid: String,
    component_version: String,
    provider: String,
    producer: String,
    input_identity: String,
    embedded_input_sha256: BTreeMap<String, String>,
    managed_destination: String,
    upstream_sha1: Option<String>,
    upstream_sha256: Option<String>,
    computed_sha256: String,
    digest_origin: String,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct FileHashes {
    bytes: u64,
    sha1: [u8; 20],
    sha256: [u8; 32],
}

pub(crate) fn reusable_generated_output(
    data_root: &DataRoot,
    output: &GeneratedOutput,
    component: &ResolvedComponent,
    embedded_input_sha256: &BTreeMap<String, String>,
    cancellation: &CancellationToken,
) -> Result<bool> {
    if !matches!(output.scope, GeneratedOutputScope::SharedImmutable) {
        return Ok(false);
    }

    let destination = managed_under(data_root.path(), &output.managed_destination)?;
    let hashes = match hash_ordinary_file(&destination, cancellation) {
        Ok(value) => value,
        Err(error) if error.code == ErrorCode::LoaderProcessorOutputMissing => return Ok(false),
        Err(error) => return Err(error),
    };
    if !matches_expected(&hashes, &output.expected_integrity, output.expected_size) {
        return Ok(false);
    }

    let provenance_path = provenance_path(data_root.path(), output, component)?;
    let metadata = match fs::symlink_metadata(&provenance_path) {
        Ok(value)
            if value.is_file()
                && !value.file_type().is_symlink()
                && value.len() <= MAX_PROVENANCE_BYTES =>
        {
            value
        }
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to inspect generated artifact provenance",
            )
            .with_source(source));
        }
    };

    let _ = metadata;
    let bytes = fs::read(&provenance_path).map_err(|source| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "failed to read generated artifact provenance",
        )
        .with_source(source)
    })?;
    let provenance: GeneratedProvenance = serde_json::from_slice(&bytes).map_err(|source| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "generated artifact provenance is invalid",
        )
        .with_source(source)
    })?;

    Ok(provenance.schema_version == 1
        && provenance.component_uid == component.uid.as_str()
        && provenance.component_version == component.version.as_str()
        && provenance.provider == component.provenance.provider
        && provenance.producer == output.producer
        && provenance.input_identity == output.input_identity
        && provenance.embedded_input_sha256 == *embedded_input_sha256
        && provenance.managed_destination == output.managed_destination.as_str()
        && provenance.upstream_sha1
            == output
                .expected_integrity
                .sha1()
                .map(|digest| digest.to_string())
        && provenance.upstream_sha256
            == output
                .expected_integrity
                .sha256()
                .map(|digest| digest.to_string())
        && provenance.computed_sha256 == hex(&hashes.sha256)
        && provenance.bytes == hashes.bytes)
}

pub(crate) fn verify_and_publish_generated_output(
    data_root: &DataRoot,
    work_root: &Path,
    output: &GeneratedOutput,
    component: &ResolvedComponent,
    embedded_input_sha256: &BTreeMap<String, String>,
    cancellation: &CancellationToken,
) -> Result<()> {
    checkpoint(cancellation)?;
    let source_relative = minecraft_path_to_platform(&output.staging_path)?;
    let source = source_relative.under(work_root);
    let hashes = hash_ordinary_file(&source, cancellation)?;

    if !matches_expected(&hashes, &output.expected_integrity, output.expected_size) {
        return Err(install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "loader processor generated output failed integrity verification",
        )
        .with_context("output", output.id.clone()));
    }

    match output.scope {
        GeneratedOutputScope::StagingOnly => Ok(()),
        GeneratedOutputScope::InstanceStaging => Err(install_error(
            ErrorCode::LoaderProcessorUnsupported,
            "instance-staging generated outputs require an instance publication context",
        )),
        GeneratedOutputScope::SharedImmutable => {
            let destination = managed_under(data_root.path(), &output.managed_destination)?;
            publish_file(&source, &destination, &hashes, cancellation)?;
            let provenance = GeneratedProvenance {
                schema_version: 1,
                component_uid: component.uid.as_str().to_owned(),
                component_version: component.version.as_str().to_owned(),
                provider: component.provenance.provider.clone(),
                producer: output.producer.clone(),
                input_identity: output.input_identity.clone(),
                embedded_input_sha256: embedded_input_sha256.clone(),
                managed_destination: output.managed_destination.as_str().to_owned(),
                upstream_sha1: output
                    .expected_integrity
                    .sha1()
                    .map(|value| value.to_string()),
                upstream_sha256: output
                    .expected_integrity
                    .sha256()
                    .map(|value| value.to_string()),
                computed_sha256: hex(&hashes.sha256),
                digest_origin: if output.expected_integrity.is_verifiable() {
                    "upstream-verified-output-with-local-sha256"
                } else {
                    "locally-derived-sha256"
                }
                .to_owned(),
                bytes: hashes.bytes,
            };
            write_provenance(
                data_root.path(),
                output,
                component,
                &provenance,
                cancellation,
            )
        }
        _ => Err(install_error(
            ErrorCode::LoaderProcessorUnsupported,
            "generated output publication scope is unsupported",
        )),
    }
}

fn managed_under(root: &Path, value: &graphene_minecraft::ManagedPath) -> Result<PathBuf> {
    let relative = minecraft_path_to_platform(value)?;
    Ok(relative.under(root))
}

pub(crate) fn locally_derived_sha256(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<String> {
    hash_ordinary_file(path, cancellation).map(|hashes| hex(&hashes.sha256))
}

fn hash_ordinary_file(path: &Path, cancellation: &CancellationToken) -> Result<FileHashes> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(install_error(
                ErrorCode::LoaderProcessorOutputMissing,
                "declared loader processor output is missing",
            ));
        }
        Err(source) => {
            return Err(install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to inspect loader processor output",
            )
            .with_source(source));
        }
    };

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "loader processor output is not an ordinary file",
        ));
    }

    if metadata.len() > MAX_GENERATED_OUTPUT_BYTES {
        return Err(install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "loader processor output exceeds the generated-artifact size bound",
        ));
    }

    let mut file = fs::File::open(path).map_err(|source| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "failed to open loader processor output",
        )
        .with_source(source)
    })?;
    let mut sha1 = Sha1::new();
    let mut sha256 = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];

    loop {
        checkpoint(cancellation)?;
        let read = file.read(&mut buffer).map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to read loader processor output",
            )
            .with_source(source)
        })?;
        if read == 0 {
            break;
        }

        bytes = bytes.saturating_add(read as u64);
        sha1.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
    }

    Ok(FileHashes {
        bytes,
        sha1: sha1.finalize().into(),
        sha256: sha256.finalize().into(),
    })
}

fn matches_expected(
    hashes: &FileHashes,
    integrity: &ArtifactIntegrity,
    expected_size: Option<u64>,
) -> bool {
    if expected_size.is_some_and(|expected| expected != hashes.bytes) {
        return false;
    }

    if integrity
        .sha1()
        .is_some_and(|expected| expected.as_bytes() != &hashes.sha1)
    {
        return false;
    }

    if integrity
        .sha256()
        .is_some_and(|expected| expected.as_bytes() != &hashes.sha256)
    {
        return false;
    }

    true
}

fn publish_file(
    source: &Path,
    destination: &Path,
    hashes: &FileHashes,
    cancellation: &CancellationToken,
) -> Result<()> {
    checkpoint(cancellation)?;
    let parent = destination.parent().ok_or_else(|| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "generated artifact destination has no parent",
        )
    })?;

    fs::create_dir_all(parent).map_err(|source| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "failed to create generated artifact destination directory",
        )
        .with_source(source)
    })?;

    let temporary =
        destination.with_extension(format!("graphene-{}.tmp", graphene_core::ArtifactId::new()));
    let result = (|| {
        let mut input = fs::File::open(source).map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to reopen generated output",
            )
            .with_source(source)
        })?;

        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| {
                install_error(
                    ErrorCode::LoaderProcessorOutputMismatch,
                    "failed to create generated output publication temporary",
                )
                .with_source(source)
            })?;

        let mut buffer = [0_u8; HASH_BUFFER_BYTES];

        loop {
            checkpoint(cancellation)?;
            let read = input.read(&mut buffer).map_err(|source| {
                install_error(
                    ErrorCode::LoaderProcessorOutputMismatch,
                    "failed to copy generated output",
                )
                .with_source(source)
            })?;
            if read == 0 {
                break;
            }

            output.write_all(&buffer[..read]).map_err(|source| {
                install_error(
                    ErrorCode::LoaderProcessorOutputMismatch,
                    "failed to write generated output publication temporary",
                )
                .with_source(source)
            })?;
        }

        output.sync_all().map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to sync generated output publication temporary",
            )
            .with_source(source)
        })?;

        drop(output);

        let copied = hash_ordinary_file(&temporary, cancellation)?;
        if copied.bytes != hashes.bytes
            || copied.sha1 != hashes.sha1
            || copied.sha256 != hashes.sha256
        {
            return Err(install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "generated output changed during publication",
            ));
        }

        replace_file_safely(&temporary, destination).map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to atomically publish generated output",
            )
            .with_source(source)
        })
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result
}

fn write_provenance(
    root: &Path,
    output: &GeneratedOutput,
    component: &ResolvedComponent,
    provenance: &GeneratedProvenance,
    cancellation: &CancellationToken,
) -> Result<()> {
    checkpoint(cancellation)?;

    let path = provenance_path(root, output, component)?;
    let bytes = serde_json::to_vec_pretty(provenance).map_err(|source| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "failed to serialize generated artifact provenance",
        )
        .with_source(source)
    })?;

    if bytes.len() as u64 > MAX_PROVENANCE_BYTES {
        return Err(install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "generated artifact provenance exceeds its bound",
        ));
    }

    let parent = path.parent().ok_or_else(|| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "generated provenance path has no parent",
        )
    })?;

    fs::create_dir_all(parent).map_err(|source| {
        install_error(
            ErrorCode::LoaderProcessorOutputMismatch,
            "failed to create generated provenance directory",
        )
        .with_source(source)
    })?;

    let temporary =
        path.with_extension(format!("graphene-{}.tmp", graphene_core::ArtifactId::new()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| {
                install_error(
                    ErrorCode::LoaderProcessorOutputMismatch,
                    "failed to create generated provenance temporary",
                )
                .with_source(source)
            })?;

        file.write_all(&bytes).map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to write generated provenance",
            )
            .with_source(source)
        })?;

        file.sync_all().map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to sync generated provenance",
            )
            .with_source(source)
        })?;

        drop(file);

        replace_file_safely(&temporary, &path).map_err(|source| {
            install_error(
                ErrorCode::LoaderProcessorOutputMismatch,
                "failed to atomically publish generated provenance",
            )
            .with_source(source)
        })
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result
}

fn provenance_path(
    root: &Path,
    output: &GeneratedOutput,
    component: &ResolvedComponent,
) -> Result<PathBuf> {
    let key = format!(
        "{}\0{}\0{}\0{}",
        component.uid.as_str(),
        component.version.as_str(),
        output.producer,
        output.managed_destination.as_str()
    );
    let digest: [u8; 32] = Sha256::digest(key.as_bytes()).into();
    let relative =
        ManagedRelativePath::new(format!("shared/metadata/generated/{}.json", hex(&digest)))?;

    Ok(relative.under(root))
}

fn checkpoint(cancellation: &CancellationToken) -> Result<()> {
    if cancellation.is_cancelled() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }

    output
}

#[cfg(test)]
mod tests;
