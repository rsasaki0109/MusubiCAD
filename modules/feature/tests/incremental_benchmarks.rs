//! Incremental regeneration call-count benchmarks (MCAD-P6-002).
//!
//! Synthetic chains of 22, 100, and 250 feature nodes gate deterministic call
//! counts and cold/incremental equivalence. Wall time is informational; CI
//! gates call counts, not noisy timing.

use opencad_core::{ConstraintId, EntityId, Expression, Length, Result, SketchId};
use opencad_feature::{FeatureRegistry, PartModel, RegenerationCache};
use opencad_geometry::{ExtrudeExtent, MockGeometryKernel};
use opencad_graph::{evaluate_param_graph, ParamGraph, ParameterEntry};
use opencad_sketch::{
    constraint::{Constraint, DistanceTarget},
    entity::{Coord, EntityBase, LineEntity, PointEntity, SketchEntity},
    workplane::Workplane,
    Sketch,
};

fn rectangle_sketch(id: &str, width_expr: &str, length_expr: &str) -> Result<Sketch> {
    let mut sketch = Sketch::new(SketchId::new(id)?, "Chain Link", Workplane::xy());
    for (corner, x, y) in [
        ("c0", 0.0, 0.0),
        ("c1", 0.0, 0.05),
        ("c2", 0.05, 0.05),
        ("c3", 0.05, 0.0),
    ] {
        sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: EntityId::new(format!("ent:{id}_{corner}"))?,
                construction: false,
            },
            x: Coord::literal(x),
            y: Coord::literal(y),
        }))?;
    }
    for (edge, start, end) in [
        ("e0", "c0", "c1"),
        ("e1", "c1", "c2"),
        ("e2", "c2", "c3"),
        ("e3", "c3", "c0"),
    ] {
        sketch.add_entity(SketchEntity::Line(LineEntity {
            base: EntityBase {
                id: EntityId::new(format!("ent:{id}_{edge}"))?,
                construction: false,
            },
            start: EntityId::new(format!("ent:{id}_{start}"))?,
            end: EntityId::new(format!("ent:{id}_{end}"))?,
        }))?;
    }
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{id}_length"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(format!("ent:{id}_e0"))?,
        },
        expr: Expression::new(length_expr)?,
    })?;
    sketch.add_constraint(Constraint::Distance {
        id: ConstraintId::new(format!("con:{id}_width"))?,
        target: DistanceTarget::LineLength {
            line: EntityId::new(format!("ent:{id}_e1"))?,
        },
        expr: Expression::new(width_expr)?,
    })?;
    Ok(sketch)
}

/// A chain of `links` extrudes (`2 * links` feature nodes). Every link shares
/// the `width` and `length` parameters; the last link uses `tail_length`.
fn chain_model(links: usize) -> Result<PartModel> {
    use opencad_feature::{ExtrudeFeature, FeatureDefinition, FeatureNode, SketchFeatureDef};

    let mut model = PartModel::new();
    for index in 0..links {
        let length_expr = if index + 1 == links {
            "tail_length"
        } else {
            "length"
        };
        let sketch = rectangle_sketch(&format!("sketch:link_{index}"), "width", length_expr)?;
        let sketch_id = sketch.id.as_str().to_string();
        model.sketches.insert(sketch_id.clone(), sketch);
        model.add_node(FeatureNode::new(
            format!("feature:sketch_link_{index}"),
            "Chain Link Sketch",
            FeatureDefinition::Sketch(SketchFeatureDef {
                sketch_id: sketch_id.clone(),
            }),
        ))?;

        let (operation, target) = if index == 0 {
            (opencad_geometry::ExtrudeOperation::NewBody, None)
        } else {
            (
                opencad_geometry::ExtrudeOperation::Join,
                Some(format!("feature:extrude_link_{}", index - 1)),
            )
        };
        model.add_node(FeatureNode::new(
            format!("feature:extrude_link_{index}"),
            "Chain Link Body",
            FeatureDefinition::Extrude(ExtrudeFeature {
                sketch_feature: format!("feature:sketch_link_{index}"),
                profile_ref: format!("{sketch_id}/profile:outer"),
                extent: ExtrudeExtent::Distance {
                    length: Length::from_meters(0.010),
                },
                operation,
                length_expr: None,
                target_feature: target,
            }),
        ))?;
        model.add_dependency(
            &format!("feature:sketch_link_{index}"),
            &format!("feature:extrude_link_{index}"),
        )?;
        if index > 0 {
            model.add_dependency(
                &format!("feature:extrude_link_{}", index - 1),
                &format!("feature:extrude_link_{index}"),
            )?;
        }
    }
    Ok(model)
}

fn chain_parameters() -> Result<ParamGraph> {
    let mut graph = ParamGraph::new();
    for (id, name, expr) in [
        ("param:width", "width", "30 mm"),
        ("param:length", "length", "50 mm"),
        ("param:tail_length", "tail_length", "60 mm"),
    ] {
        graph.add_parameter(ParameterEntry::new(id, name, expr))?;
    }
    Ok(graph)
}

struct ChainRun {
    nodes: usize,
    cold_kernel_calls: u64,
    warm_executed: usize,
    warm_cached: usize,
    warm_kernel_calls: u64,
    incremental_executed: usize,
    incremental_cached: usize,
}

fn run_chain(links: usize) -> Result<ChainRun> {
    let kernel = MockGeometryKernel::new();
    let registry = FeatureRegistry::with_defaults();
    let mut model = chain_model(links)?;
    let mut params = chain_parameters()?;
    let mut cache = RegenerationCache::with_backend("mock");
    let nodes = model.nodes.len();

    let cold = model.regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)?;
    assert_eq!(
        cold.regenerated.len(),
        nodes,
        "cold chain must execute every node"
    );

    let warm = model.regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)?;
    assert_eq!(
        warm.regenerated.len(),
        0,
        "unchanged chain must not re-execute"
    );
    assert_eq!(warm.cached_nodes.len(), nodes);
    assert_eq!(warm.trace.geometry_kernel_call_count, 0);

    params.set_expr("param:tail_length", "70 mm")?;
    let incremental =
        model.regenerate_with_cache(&kernel, &registry, Some(&params), None, &mut cache)?;
    assert_eq!(
        incremental.cached_nodes.len(),
        nodes - 2,
        "only the tail link (sketch + extrude) may re-execute"
    );
    assert_eq!(incremental.regenerated.len(), 2);

    Ok(ChainRun {
        nodes,
        cold_kernel_calls: cold.trace.geometry_kernel_call_count,
        warm_executed: warm.regenerated.len(),
        warm_cached: warm.cached_nodes.len(),
        warm_kernel_calls: warm.trace.geometry_kernel_call_count,
        incremental_executed: incremental.regenerated.len(),
        incremental_cached: incremental.cached_nodes.len(),
    })
}

#[test]
fn incremental_chain_benchmarks_gate_call_counts() {
    let _ = evaluate_param_graph(&chain_parameters().expect("params"));
    for links in [11usize, 50, 125] {
        let run = run_chain(links).expect("chain benchmark");
        assert_eq!(run.nodes, links * 2, "22/100/250 node fixture");
        assert!(run.cold_kernel_calls > 0, "cold chain must call the kernel");
        assert_eq!(run.warm_executed, 0);
        assert_eq!(run.warm_cached, run.nodes);
        assert_eq!(
            run.warm_kernel_calls, 0,
            "warm chain must make zero kernel calls"
        );
        assert_eq!(run.incremental_executed, 2);
        assert_eq!(run.incremental_cached, run.nodes - 2);
    }
}

#[test]
fn incremental_chain_benchmarks_are_deterministic() {
    let first = run_chain(11).expect("first run");
    let second = run_chain(11).expect("second run");
    assert_eq!(first.cold_kernel_calls, second.cold_kernel_calls);
    assert_eq!(first.warm_kernel_calls, second.warm_kernel_calls);
    assert_eq!(first.incremental_executed, second.incremental_executed);
}
