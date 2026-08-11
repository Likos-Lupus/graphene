use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct ReleaseDto {
    pub(super) binary: BinaryDto,
    pub(super) release_name: String,
    #[serde(default)]
    pub(super) vendor: String,
    pub(super) version: VersionDto,
}

#[derive(Deserialize)]
pub(super) struct BinaryDto {
    pub(super) architecture: String,
    pub(super) image_type: String,
    pub(super) os: String,
    pub(super) package: PackageDto,
}

#[derive(Deserialize)]
pub(super) struct PackageDto {
    pub(super) checksum: String,
    pub(super) link: String,
    pub(super) name: String,
    pub(super) size: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct VersionDto {
    pub(super) major: u32,
    pub(super) semver: String,
}
