use crate::{Argument, Library, MinecraftJavaRequirement, ResolvedComponent, ResolvedLogging};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinecraftVersionPatch {
    pub component: ResolvedComponent,
    pub main_class: Option<String>,
    pub libraries: Vec<Library>,
    pub jvm_args: Vec<Argument>,
    pub game_args: Vec<Argument>,
    pub java_requirement: Option<MinecraftJavaRequirement>,
    pub logging: Option<ResolvedLogging>,
}

impl MinecraftVersionPatch {
    #[must_use]
    pub fn empty(component: ResolvedComponent) -> Self {
        Self {
            component,
            main_class: None,
            libraries: Vec::new(),
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            java_requirement: None,
            logging: None,
        }
    }
}
