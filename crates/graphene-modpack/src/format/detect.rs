use crate::archive::PackArchiveIndex;
use crate::error::PackError;
use graphene_core::ErrorCode;
use std::io::Read;
use std::io::Seek;

/// Recognized manifest roots inside one archive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetectedFormatRoots {
    pub graphene_manifest: bool,
    pub modrinth_index: bool,
    pub curseforge_manifest: bool,
    pub multimc_pack: bool,
}

impl DetectedFormatRoots {
    /// Number of recognized mutually-exclusive format claims.
    #[must_use]
    pub fn recognized_count(&self) -> usize {
        usize::from(self.graphene_manifest)
            + usize::from(self.modrinth_index)
            + usize::from(self.curseforge_manifest)
            + usize::from(self.multimc_pack)
    }
}

/// Detection result: the recognized format plus the effective root prefix (wrapper directory with
/// trailing slash, or empty when entries live at the archive root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatDetection {
    pub format: crate::model::format::PackFormat,
    /// Effective root prefix including trailing slash; empty for no wrapper.
    pub root_prefix: String,
}

/// Detects the pack format by content, not extension.
///
/// One effective root is chosen deterministically: a single shared wrapper directory is stripped
/// only when every entry lives under it. Two recognized format manifests at the same effective
/// root fail as ambiguous rather than resolving by precedence.
pub fn detect_pack_format<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    allow_generic: bool,
) -> Result<FormatDetection, PackError> {
    let names: Vec<String> = index
        .entries()
        .iter()
        .filter(|entry| entry.is_regular && !entry.is_directory && !entry.is_symlink)
        .map(|entry| entry.name.clone())
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();

    let wrapper = super::super::archive::path::detect_wrapper_root(&refs);
    let root_prefix = match &wrapper {
        Some(name) => {
            // Wrapper stripping must not create ambiguity: reject nested duplicate wrappers.
            if name.matches('/').count() > 0 || name.is_empty() {
                return Err(PackError::new(
                    ErrorCode::PackFormatUnknown,
                    "pack wrapper root is ambiguous",
                ));
            }
            format!("{name}/")
        }
        None => String::new(),
    };

    // A recognized format must be found at exactly one effective root: either the wrapper root or
    // the archive root. Mixed placement is ambiguous.
    let roots = DetectedFormatRoots {
        graphene_manifest: recognizes_graphene(index, &root_prefix)?,
        modrinth_index: super::modrinth::recognizes(index, &root_prefix)?,
        curseforge_manifest: super::curseforge::recognizes(index, &root_prefix)?,
        multimc_pack: super::multimc::recognizes(index, &root_prefix)?,
    };

    let count = roots.recognized_count();
    if count > 1 {
        return Err(PackError::new(
            ErrorCode::PackFormatAmbiguous,
            "archive declares multiple pack formats",
        ));
    }

    let format = if roots.graphene_manifest {
        crate::model::format::PackFormat::Graphene
    } else if roots.modrinth_index {
        crate::model::format::PackFormat::Modrinth
    } else if roots.curseforge_manifest {
        crate::model::format::PackFormat::CurseForge
    } else if roots.multimc_pack {
        crate::model::format::PackFormat::PrismMultiMc
    } else if allow_generic {
        crate::model::format::PackFormat::Generic
    } else {
        return Err(PackError::new(
            ErrorCode::PackFormatUnknown,
            "no supported pack manifest was found",
        ));
    };

    Ok(FormatDetection {
        format,
        root_prefix,
    })
}

fn recognizes_graphene<R: Read + Seek>(
    index: &mut PackArchiveIndex<R>,
    root: &str,
) -> Result<bool, PackError> {
    let name = format!("{root}graphene.pack.json");
    if index.entry(&name).is_none() {
        return Ok(false);
    }

    super::graphene::recognizes_manifest(index.read_manifest(&name)?.as_slice())
}
