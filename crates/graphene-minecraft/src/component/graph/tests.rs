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

mod validation_matrix {
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
