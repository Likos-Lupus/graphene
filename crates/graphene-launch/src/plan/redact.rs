use super::{LaunchArgument, LaunchPlan, RedactedLaunchPlan};
use std::path::Path;

impl LaunchPlan {
    #[must_use]
    pub fn redacted_snapshot(&self, data_root: &Path) -> RedactedLaunchPlan {
        RedactedLaunchPlan {
            instance_id: self.instance_id.to_string(),
            java: normalize_snapshot_external_path(&self.java.executable, "java"),
            working_directory: normalize_snapshot_path(&self.working_directory, data_root),
            environment_keys: self.environment.values.keys().cloned().collect(),
            jvm_args: self.jvm_args.iter().map(LaunchArgument::redacted).collect(),
            classpath: self
                .classpath
                .iter()
                .map(|path| normalize_snapshot_path(path, data_root))
                .collect(),
            classpath_separator: self.classpath_separator.to_string(),
            main_class: self.main_class.clone(),
            game_args: self
                .game_args
                .iter()
                .map(LaunchArgument::redacted)
                .collect(),
            natives_directory: normalize_snapshot_path(&self.natives_directory, data_root),
        }
    }
}

fn normalize_snapshot_path(path: &Path, data_root: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(data_root) {
        format!("<root>/{}", relative.to_string_lossy().replace('\\', "/"))
    } else {
        normalize_snapshot_external_path(path, "external")
    }
}

fn normalize_snapshot_external_path(path: &Path, role: &str) -> String {
    let file = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    format!("<{role}>/{file}")
}
