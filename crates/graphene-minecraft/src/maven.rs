use crate::{ManagedPath, error::library_error};
use graphene_core::Result;
use serde::{Deserialize, Serialize};

/// Validated Maven coordinate used for library identity and path synthesis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MavenCoordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl MavenCoordinate {
    pub fn parse(input: &str) -> Result<Self> {
        if input.is_empty() || input.len() > 512 {
            return Err(library_error("Maven coordinate is empty or too long"));
        }

        let (base, extension) = match input.rsplit_once('@') {
            Some((base, extension)) => (base, extension),
            None => (input, "jar"),
        };
        let fields: Vec<&str> = base.split(':').collect();

        if !(3..=4).contains(&fields.len()) {
            return Err(library_error(
                "Maven coordinate has an unsupported field count",
            ));
        }

        for field in fields.iter().copied().chain(std::iter::once(extension)) {
            validate_maven_component(field)?;
        }

        Ok(Self {
            group: fields[0].to_owned(),
            artifact: fields[1].to_owned(),
            version: fields[2].to_owned(),
            classifier: fields.get(3).map(|value| (*value).to_owned()),
            extension: extension.to_owned(),
        })
    }

    #[must_use]
    pub fn library_identity(&self) -> String {
        format!("{}:{}", self.group, self.artifact)
    }

    pub fn repository_path(&self) -> Result<ManagedPath> {
        let group = self.group.replace('.', "/");
        let classifier = self
            .classifier
            .as_ref()
            .map(|value| format!("-{value}"))
            .unwrap_or_default();
        ManagedPath::new(format!(
            "{group}/{}/{}/{}-{}{}.{}",
            self.artifact, self.version, self.artifact, self.version, classifier, self.extension
        ))
    }
}

fn validate_maven_component(value: &str) -> Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.contains('\0')
    {
        return Err(library_error(
            "Maven coordinate contains an unsafe component",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::ErrorCode;

    #[test]
    fn maven_coordinates_parse_and_reject_unsafe_components() {
        let coordinate =
            MavenCoordinate::parse("org.lwjgl:lwjgl:3.3.3:natives-linux@jar").expect("coordinate");
        assert_eq!(coordinate.group, "org.lwjgl");
        assert_eq!(
            coordinate.repository_path().expect("path").as_str(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
        );
        assert_eq!(
            MavenCoordinate::parse("org.bad/escape:lib:1")
                .expect_err("unsafe")
                .code,
            ErrorCode::MinecraftLibraryInvalid
        );
    }
}
