use super::{ComponentDescriptor, ComponentKind, ComponentRequirement};
use crate::error::mc_error;
use graphene_core::{ErrorCode, Result};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_COMPONENT_NODES: usize = 32;
pub const MAX_COMPONENT_EDGES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentGraph {
    nodes: Vec<ComponentDescriptor>,
    order: Vec<usize>,
}

impl ComponentGraph {
    pub fn new(nodes: Vec<ComponentDescriptor>) -> Result<Self> {
        if nodes.is_empty() || nodes.len() > MAX_COMPONENT_NODES {
            return Err(graph_error("component graph node count is invalid"));
        }

        let mut by_uid = BTreeMap::new();
        let mut minecraft = 0usize;
        let mut loaders = 0usize;
        let mut edge_count = 0usize;

        for (index, node) in nodes.iter().enumerate() {
            if by_uid.insert(node.uid.clone(), index).is_some() {
                return Err(graph_error("component graph contains duplicate UIDs")
                    .with_context("component_uid", node.uid.to_string()));
            }

            match node.kind {
                ComponentKind::Minecraft => minecraft += 1,
                ComponentKind::Loader => loaders += 1,
                ComponentKind::Auxiliary => {}
            }

            edge_count = edge_count.saturating_add(node.requires.len());
            if edge_count > MAX_COMPONENT_EDGES {
                return Err(graph_error("component graph contains too many edges"));
            }
        }

        if minecraft != 1 {
            return Err(graph_error(
                "component graph must contain exactly one Minecraft component",
            ));
        }

        if loaders > 1 {
            return Err(mc_error(
                ErrorCode::ComponentConflict,
                "component graph contains multiple primary loaders",
            ));
        }

        for node in &nodes {
            for requirement in &node.requires {
                let Some(required_index) = by_uid.get(requirement.uid()).copied() else {
                    return Err(mc_error(
                        ErrorCode::ComponentRequirementUnsatisfied,
                        "component requirement is unsatisfied",
                    )
                    .with_context("component_uid", node.uid.to_string())
                    .with_context("required_uid", requirement.uid().to_string()));
                };

                if let ComponentRequirement::Exact { version, .. } = requirement
                    && nodes[required_index].version != *version
                {
                    return Err(mc_error(
                        ErrorCode::ComponentRequirementUnsatisfied,
                        "component exact-version requirement is unsatisfied",
                    )
                    .with_context("component_uid", node.uid.to_string())
                    .with_context("required_uid", requirement.uid().to_string())
                    .with_context("required_version", version.to_string()));
                }
            }

            for conflict in &node.conflicts {
                if by_uid.contains_key(&conflict.uid) {
                    return Err(mc_error(
                        ErrorCode::ComponentConflict,
                        "component graph contains a declared conflict",
                    )
                    .with_context("component_uid", node.uid.to_string())
                    .with_context("conflicting_uid", conflict.uid.to_string()));
                }
            }
        }

        let mut incoming = vec![0usize; nodes.len()];
        let mut outgoing = vec![Vec::<usize>::new(); nodes.len()];
        for (dependent, node) in nodes.iter().enumerate() {
            for requirement in &node.requires {
                let dependency = by_uid[requirement.uid()];
                outgoing[dependency].push(dependent);
                incoming[dependent] += 1;
            }
        }

        let mut eligible = BTreeSet::<(i32, String, String, usize)>::new();
        for (index, node) in nodes.iter().enumerate() {
            if incoming[index] == 0 {
                eligible.insert(sort_key(node, index));
            }
        }

        let mut order = Vec::with_capacity(nodes.len());
        while let Some(key) = eligible.pop_first() {
            let index = key.3;
            order.push(index);
            let mut dependents = outgoing[index].clone();
            dependents.sort_unstable();

            for dependent in dependents {
                incoming[dependent] -= 1;
                if incoming[dependent] == 0 {
                    eligible.insert(sort_key(&nodes[dependent], dependent));
                }
            }
        }

        if order.len() != nodes.len() {
            let cycle = nodes
                .iter()
                .enumerate()
                .filter(|(index, _)| incoming[*index] > 0)
                .map(|(_, node)| node.uid.as_str())
                .collect::<Vec<_>>()
                .join(",");

            return Err(mc_error(
                ErrorCode::ComponentCycle,
                "component graph contains a dependency cycle",
            )
            .with_context("components", cycle));
        }

        Ok(Self { nodes, order })
    }

    #[must_use]
    pub fn nodes(&self) -> &[ComponentDescriptor] {
        &self.nodes
    }

    pub fn ordered(&self) -> impl Iterator<Item = &ComponentDescriptor> {
        self.order.iter().map(|index| &self.nodes[*index])
    }
}

fn sort_key(node: &ComponentDescriptor, index: usize) -> (i32, String, String, usize) {
    (
        node.order,
        node.uid.as_str().to_owned(),
        node.version.as_str().to_owned(),
        index,
    )
}

fn graph_error(message: &'static str) -> graphene_core::GrapheneError {
    mc_error(ErrorCode::ComponentGraphInvalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ComponentConflict, ComponentUid, ComponentVersion};

    fn node(uid: &str, kind: ComponentKind, order: i32) -> ComponentDescriptor {
        ComponentDescriptor {
            uid: ComponentUid::new(uid).expect("uid"),
            version: ComponentVersion::new("1").expect("version"),
            kind,
            order,
            requires: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    #[test]
    fn deterministic_topological_order_uses_order_then_uid() {
        let base = node("net.minecraft", ComponentKind::Minecraft, 0);
        let mut z = node("z.example", ComponentKind::Auxiliary, 10);
        z.requires.push(ComponentRequirement::Any {
            uid: base.uid.clone(),
        });
        let mut a = node("a.example", ComponentKind::Auxiliary, 10);
        a.requires.push(ComponentRequirement::Any {
            uid: base.uid.clone(),
        });
        let graph = ComponentGraph::new(vec![z, base, a]).expect("graph");
        let order = graph.ordered().map(|n| n.uid.as_str()).collect::<Vec<_>>();
        assert_eq!(order, vec!["net.minecraft", "a.example", "z.example"]);
    }

    #[test]
    fn cycles_and_multiple_loaders_are_rejected() {
        let base = node("net.minecraft", ComponentKind::Minecraft, 0);
        let mut a = node("a.example", ComponentKind::Auxiliary, 1);
        let mut b = node("b.example", ComponentKind::Auxiliary, 2);
        a.requires
            .push(ComponentRequirement::Any { uid: b.uid.clone() });
        b.requires
            .push(ComponentRequirement::Any { uid: a.uid.clone() });
        let error = ComponentGraph::new(vec![base.clone(), a, b]).expect_err("cycle");
        assert_eq!(error.code, ErrorCode::ComponentCycle);

        let f = node("net.fabricmc.fabric-loader", ComponentKind::Loader, 10);
        let g = node("net.minecraftforge.forge", ComponentKind::Loader, 10);
        let error = ComponentGraph::new(vec![base, f, g]).expect_err("loader conflict");
        assert_eq!(error.code, ErrorCode::ComponentConflict);
    }

    #[test]
    fn declared_conflicts_are_structural() {
        let base = node("net.minecraft", ComponentKind::Minecraft, 0);
        let mut aux = node("a.example", ComponentKind::Auxiliary, 1);
        aux.conflicts.push(ComponentConflict {
            uid: base.uid.clone(),
        });
        assert_eq!(
            ComponentGraph::new(vec![base, aux])
                .expect_err("conflict")
                .code,
            ErrorCode::ComponentConflict
        );
    }
}

#[cfg(test)]
mod phase3_matrix_tests {
    use super::*;
    use crate::{ComponentKind, ComponentRequirement, ComponentUid, ComponentVersion};

    fn node(uid: &str, kind: ComponentKind) -> ComponentDescriptor {
        ComponentDescriptor {
            uid: ComponentUid::new(uid).expect("uid"),
            version: ComponentVersion::new("1").expect("version"),
            kind,
            order: 0,
            requires: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    #[test]
    fn missing_duplicate_and_multiple_base_components_fail() {
        let aux = node("example.aux", ComponentKind::Auxiliary);
        assert_eq!(
            ComponentGraph::new(vec![aux])
                .expect_err("missing base")
                .code,
            ErrorCode::ComponentGraphInvalid
        );

        let base = node("net.minecraft", ComponentKind::Minecraft);
        assert_eq!(
            ComponentGraph::new(vec![base.clone(), base.clone()])
                .expect_err("duplicate uid")
                .code,
            ErrorCode::ComponentGraphInvalid
        );

        let other_base = node("example.minecraft", ComponentKind::Minecraft);
        assert_eq!(
            ComponentGraph::new(vec![base, other_base])
                .expect_err("multiple base")
                .code,
            ErrorCode::ComponentGraphInvalid
        );
    }

    #[test]
    fn exact_and_any_requirements_are_checked() {
        let base = node("net.minecraft", ComponentKind::Minecraft);
        let mut any = node("example.any", ComponentKind::Auxiliary);
        any.requires.push(ComponentRequirement::Any {
            uid: base.uid.clone(),
        });
        assert!(ComponentGraph::new(vec![base.clone(), any]).is_ok());

        let mut exact = node("example.exact", ComponentKind::Auxiliary);
        exact.requires.push(ComponentRequirement::Exact {
            uid: base.uid.clone(),
            version: ComponentVersion::new("2").expect("version"),
        });
        assert_eq!(
            ComponentGraph::new(vec![base.clone(), exact])
                .expect_err("version mismatch")
                .code,
            ErrorCode::ComponentRequirementUnsatisfied
        );

        let mut missing = node("example.missing", ComponentKind::Auxiliary);
        missing.requires.push(ComponentRequirement::Any {
            uid: ComponentUid::new("not.present").expect("uid"),
        });
        assert_eq!(
            ComponentGraph::new(vec![base, missing])
                .expect_err("missing requirement")
                .code,
            ErrorCode::ComponentRequirementUnsatisfied
        );
    }

    #[test]
    fn node_and_edge_bounds_fail_before_graph_execution() {
        let mut nodes = vec![node("net.minecraft", ComponentKind::Minecraft)];
        for index in 0..MAX_COMPONENT_NODES {
            nodes.push(node(&format!("example.n{index}"), ComponentKind::Auxiliary));
        }

        assert_eq!(
            ComponentGraph::new(nodes).expect_err("node bound").code,
            ErrorCode::ComponentGraphInvalid
        );

        let base = node("net.minecraft", ComponentKind::Minecraft);
        let mut aux = node("example.edges", ComponentKind::Auxiliary);
        aux.requires = (0..=MAX_COMPONENT_EDGES)
            .map(|_| ComponentRequirement::Any {
                uid: base.uid.clone(),
            })
            .collect();
        assert_eq!(
            ComponentGraph::new(vec![base, aux])
                .expect_err("edge bound")
                .code,
            ErrorCode::ComponentGraphInvalid
        );
    }
}
