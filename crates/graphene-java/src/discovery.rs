use crate::{JavaCandidate, JavaCandidateSource, error::java_error};
use graphene_core::{ErrorCode, Result};
use graphene_platform::normalize_process_path;
use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

pub const MAX_JAVA_CANDIDATES: usize = 128;

/// Discovers a bounded deterministic set of local Java executables without recursive tree walks.
pub async fn discover_java_candidates(explicit: Option<PathBuf>) -> Result<Vec<JavaCandidate>> {
    tokio::task::spawn_blocking(move || discover_java_candidates_blocking(explicit))
        .await
        .map_err(|source| {
            java_error(ErrorCode::JavaProbeFailed, "Java discovery worker failed")
                .with_source(source)
        })?
}

fn discover_java_candidates_blocking(explicit: Option<PathBuf>) -> Result<Vec<JavaCandidate>> {
    let mut candidates = Vec::new();

    if let Some(path) = explicit {
        push_candidate(&mut candidates, path, JavaCandidateSource::Explicit);
    }

    if let Some(home) = std::env::var_os("JAVA_HOME") {
        push_candidate(
            &mut candidates,
            PathBuf::from(home).join("bin").join(java_executable_name()),
            JavaCandidateSource::JavaHome,
        );
    }

    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path).take(MAX_JAVA_CANDIDATES) {
            push_candidate(
                &mut candidates,
                directory.join(java_executable_name()),
                JavaCandidateSource::Path,
            );
        }
    }

    discover_common_roots(&mut candidates);
    deduplicate_candidates(candidates)
}

fn push_candidate(candidates: &mut Vec<JavaCandidate>, path: PathBuf, source: JavaCandidateSource) {
    if candidates.len() < MAX_JAVA_CANDIDATES * 4 {
        candidates.push(JavaCandidate {
            executable: path,
            source,
        });
    }
}

fn discover_common_roots(candidates: &mut Vec<JavaCandidate>) {
    #[cfg(target_os = "linux")]
    add_java_homes_from_directory(candidates, Path::new("/usr/lib/jvm"), |path| {
        path.join("bin/java")
    });

    #[cfg(target_os = "macos")]
    add_java_homes_from_directory(
        candidates,
        Path::new("/Library/Java/JavaVirtualMachines"),
        |path| path.join("Contents/Home/bin/java"),
    );

    #[cfg(target_os = "windows")]
    {
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = std::env::var_os(variable) {
                for vendor in ["Java", "Eclipse Adoptium", "Microsoft"] {
                    add_java_homes_from_directory(
                        candidates,
                        &PathBuf::from(base.clone()).join(vendor),
                        |path| path.join("bin/java.exe"),
                    );
                }
            }
        }
    }
}

fn add_java_homes_from_directory<F>(candidates: &mut Vec<JavaCandidate>, root: &Path, executable: F)
where
    F: Fn(&Path) -> PathBuf,
{
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    let mut paths = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths.into_iter().take(MAX_JAVA_CANDIDATES) {
        push_candidate(
            candidates,
            executable(&path),
            JavaCandidateSource::CommonRoot,
        );
    }
}

fn deduplicate_candidates(mut candidates: Vec<JavaCandidate>) -> Result<Vec<JavaCandidate>> {
    let mut normalized = Vec::new();
    for mut candidate in candidates.drain(..) {
        let canonical = match fs::canonicalize(&candidate.executable) {
            Ok(path) => path,
            Err(_) => continue,
        };

        let metadata = match fs::metadata(&canonical) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if !metadata.is_file() {
            continue;
        }

        candidate.executable = normalize_process_path(canonical);
        normalized.push(candidate);
    }

    normalized.sort_by(|left, right| {
        left.source
            .priority()
            .cmp(&right.source.priority())
            .then_with(|| path_sort_key(&left.executable).cmp(&path_sort_key(&right.executable)))
    });

    let mut seen = HashSet::new();
    normalized.retain(|candidate| seen.insert(path_sort_key(&candidate.executable)));
    normalized.truncate(MAX_JAVA_CANDIDATES);
    Ok(normalized)
}

pub(crate) fn path_sort_key(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        value.to_ascii_lowercase()
    } else {
        value
    }
}

fn java_executable_name() -> OsString {
    if cfg!(windows) {
        OsString::from("java.exe")
    } else {
        OsString::from("java")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_candidates_keep_highest_priority_source() {
        let root = std::env::temp_dir().join(format!(
            "graphene-java-dedup-{}",
            graphene_core::ArtifactId::new()
        ));

        fs::create_dir(&root).expect("create root");
        let executable = root.join(java_executable_name());
        fs::write(&executable, b"fixture").expect("write candidate");
        let candidates = vec![
            JavaCandidate {
                executable: executable.clone(),
                source: JavaCandidateSource::Path,
            },
            JavaCandidate {
                executable: executable.clone(),
                source: JavaCandidateSource::Explicit,
            },
            JavaCandidate {
                executable,
                source: JavaCandidateSource::JavaHome,
            },
        ];

        let result = deduplicate_candidates(candidates).expect("deduplicate");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, JavaCandidateSource::Explicit);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
