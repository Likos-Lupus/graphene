use crate::{
    JavaArchitecture, JavaCandidate, JavaRuntime, error::java_error, normalize_vendor,
    parse_java_major,
};
use graphene_core::{ErrorCode, Result};
use graphene_platform::{ProcessSpec, run_process_bounded};
use std::{path::PathBuf, time::Duration};

pub const JAVA_PROBE_OUTPUT_LIMIT: usize = 64 * 1024;
pub const JAVA_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn probe_java(candidate: &JavaCandidate) -> Result<JavaRuntime> {
    let spec =
        ProcessSpec::new(&candidate.executable).args(["-XshowSettings:properties", "-version"]);
    let output = run_process_bounded(&spec, JAVA_PROBE_TIMEOUT, JAVA_PROBE_OUTPUT_LIMIT)
        .await
        .map_err(|source| {
            java_error(ErrorCode::JavaProbeFailed, "failed to execute Java probe")
                .with_source(source)
        })?;

    if output.timed_out {
        return Err(java_error(
            ErrorCode::JavaProbeTimeout,
            "Java probe exceeded its timeout",
        ));
    }

    if output.stdout_truncated || output.stderr_truncated {
        return Err(java_error(
            ErrorCode::JavaProbeFailed,
            "Java probe output exceeded its bound",
        ));
    }

    if output.exit.is_none_or(|exit| !exit.success) {
        return Err(java_error(
            ErrorCode::JavaProbeFailed,
            "Java probe exited unsuccessfully",
        ));
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    parse_probe_output(candidate.executable.clone(), &text)
}

pub(crate) fn parse_probe_output(executable: PathBuf, text: &str) -> Result<JavaRuntime> {
    let mut version = None::<String>;
    let mut vendor = None::<String>;
    let mut architecture = None::<String>;
    let mut java_home = None::<PathBuf>;

    for line in text.lines().take(4096) {
        let line = line.trim();

        if let Some((key, value)) = line.split_once(" = ") {
            match key.trim() {
                "java.version" => version = Some(value.trim().to_owned()),
                "java.vendor" | "java.vendor.version" if vendor.is_none() => {
                    vendor = Some(value.trim().to_owned())
                }
                "os.arch" => architecture = Some(value.trim().to_owned()),
                "java.home" => java_home = Some(PathBuf::from(value.trim())),
                _ => {}
            }
        }

        if version.is_none()
            && line.contains(" version \"")
            && let Some((_, rest)) = line.split_once(" version \"")
            && let Some((found, _)) = rest.split_once('"')
        {
            version = Some(found.to_owned());
        }
    }

    let version = version.ok_or_else(|| {
        java_error(
            ErrorCode::JavaProbeFailed,
            "Java probe did not report a version",
        )
    })?;
    let major_version = parse_java_major(&version)?;

    Ok(JavaRuntime {
        executable,
        version,
        major_version,
        vendor: normalize_vendor(vendor.as_deref().unwrap_or("unknown")),
        architecture: JavaArchitecture::normalize(architecture.as_deref().unwrap_or("unknown")),
        java_home,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JavaArchitecture, JavaVendor};

    #[test]
    fn probe_output_normalizes_properties() {
        let runtime = parse_probe_output(
            PathBuf::from("java"),
            "java.version = 1.8.0_402\njava.vendor = Eclipse Adoptium\nos.arch = amd64\njava.home = /fixture/java",
        )
        .expect("runtime");

        assert_eq!(runtime.major_version, 8);
        assert_eq!(runtime.vendor, JavaVendor::Adoptium);
        assert_eq!(runtime.architecture, JavaArchitecture::X86_64);
    }
}
