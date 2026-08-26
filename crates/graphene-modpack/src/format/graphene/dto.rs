use serde::Deserialize;

/// Strict Graphene pack manifest v1 payload.
///
/// Unknown fields are rejected so forward-incompatible manifests fail cleanly instead of being
/// parsed best-effort under unknown semantics.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphenePackDto {
    pub(super) schema_version: u64,
    pub(super) pack: GraphenePackInfoDto,
    pub(super) runtime: GrapheneRuntimeDto,
    #[serde(default)]
    pub(super) managed_files: Vec<GrapheneManagedFileDto>,
    #[serde(default)]
    pub(super) seed_files: Vec<GrapheneSeedFileDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphenePackInfoDto {
    pub(super) name: String,
    #[serde(default)]
    pub(super) version: Option<String>,
    #[serde(default)]
    pub(super) summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrapheneRuntimeDto {
    pub(super) minecraft_version: String,
    #[serde(default)]
    pub(super) primary_loader: Option<GrapheneLoaderDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrapheneLoaderDto {
    pub(super) kind: String,
    pub(super) exact_version: String,
}

/// Exactly one acquisition strategy must be declared per managed file; the normalizer enforces
/// that embedded/provenance/sources are mutually exclusive.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrapheneManagedFileDto {
    pub(super) destination: String,
    pub(super) sha256: String,
    pub(super) size: u64,
    #[serde(default)]
    pub(super) sources: Vec<String>,
    #[serde(default)]
    pub(super) embedded_object: Option<String>,
    #[serde(default)]
    pub(super) content_provenance: Option<GrapheneProvenanceDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrapheneProvenanceDto {
    pub(super) provider: String,
    pub(super) project_id: String,
    pub(super) version_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GrapheneSeedFileDto {
    pub(super) destination: String,
    pub(super) sha256: String,
    pub(super) size: u64,
    pub(super) archive_entry: String,
}
