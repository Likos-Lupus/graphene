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
mod tests;
