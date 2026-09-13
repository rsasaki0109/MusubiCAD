//! Parameter expression evaluation.

use indexmap::IndexMap;

use opencad_core::{OpenCadError, Result};

use crate::param_graph::{ParamGraph, ParameterEntry};

/// Evaluate all parameters in dependency order.
/// Length parameters resolve to meters; angle parameters ending in `_rad` resolve to radians.
pub fn evaluate_param_graph(graph: &ParamGraph) -> Result<IndexMap<String, f64>> {
    let order = graph.evaluation_order()?;
    let mut values = IndexMap::new();
    for id in order {
        let entry = graph
            .get(&id)
            .ok_or_else(|| OpenCadError::not_found(format!("parameter '{id}'")))?;
        let value = if is_angle_parameter(&entry.name) {
            eval_angle_expr(&entry.expr, &values)?
        } else {
            eval_length_expr(&entry.expr, &values)?
        };
        values.insert(entry.name.clone(), value);
    }
    Ok(values)
}

/// Evaluate a length expression in meters using resolved parameter names.
pub fn eval_length_expr(expr: &str, scope: &IndexMap<String, f64>) -> Result<f64> {
    eval_expr(expr, scope, convert_length)
}

/// Evaluate an angle expression in radians using resolved parameter names.
pub fn eval_angle_expr(expr: &str, scope: &IndexMap<String, f64>) -> Result<f64> {
    eval_expr(expr, scope, convert_angle)
}

/// Identifier names referenced by a parametric expression (e.g. `hole_diameter / 2`).
pub fn parameter_names_in_expr(expr: &str) -> Vec<String> {
    let tokens = match tokenize(expr, |value, _| value) {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };
    let mut names = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for token in tokens {
        if let Token::Ident(ident) = token {
            if seen.insert(ident.clone()) {
                names.push(ident);
            }
        }
    }
    names
}

fn is_angle_parameter(name: &str) -> bool {
    name.ends_with("_rad") || name.ends_with("_deg") || name.contains("angle")
}

fn eval_expr(
    expr: &str,
    scope: &IndexMap<String, f64>,
    convert_unit: fn(f64, &str) -> f64,
) -> Result<f64> {
    let tokens = tokenize(expr, convert_unit)?;
    let (value, rest) = parse_expr(&tokens, scope)?;
    if !rest.is_empty() {
        return Err(OpenCadError::InvalidExpression(expr.into()));
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
}

fn tokenize(input: &str, convert_unit: fn(f64, &str) -> f64) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let mut number = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '.' {
                    number.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            let value: f64 = number
                .parse()
                .map_err(|_| OpenCadError::InvalidExpression(input.into()))?;
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            if chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                let mut unit = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '+' | '-' | '*' | '/') {
                        break;
                    }
                    unit.push(c);
                    chars.next();
                }
                tokens.push(Token::Number(convert_unit(value, &unit)));
                continue;
            }
            tokens.push(Token::Number(value));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut ident = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    ident.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token::Ident(ident));
            continue;
        }
        match ch {
            '+' => {
                tokens.push(Token::Plus);
                chars.next();
            }
            '-' => {
                tokens.push(Token::Minus);
                chars.next();
            }
            '*' => {
                tokens.push(Token::Star);
                chars.next();
            }
            '/' => {
                tokens.push(Token::Slash);
                chars.next();
            }
            _ => return Err(OpenCadError::InvalidExpression(input.into())),
        }
    }

    Ok(tokens)
}

fn parse_expr<'a>(
    tokens: &'a [Token],
    scope: &IndexMap<String, f64>,
) -> Result<(f64, &'a [Token])> {
    let (mut value, mut rest) = parse_term(tokens, scope)?;
    while let Some(token) = rest.first() {
        match token {
            Token::Plus => {
                let (rhs, next) = parse_term(&rest[1..], scope)?;
                value += rhs;
                rest = next;
            }
            Token::Minus => {
                let (rhs, next) = parse_term(&rest[1..], scope)?;
                value -= rhs;
                rest = next;
            }
            _ => break,
        }
    }
    Ok((value, rest))
}

fn parse_term<'a>(
    tokens: &'a [Token],
    scope: &IndexMap<String, f64>,
) -> Result<(f64, &'a [Token])> {
    let (mut value, mut rest) = parse_factor(tokens, scope)?;
    while let Some(token) = rest.first() {
        match token {
            Token::Star => {
                let (rhs, next) = parse_factor(&rest[1..], scope)?;
                value *= rhs;
                rest = next;
            }
            Token::Slash => {
                let (rhs, next) = parse_factor(&rest[1..], scope)?;
                if rhs.abs() <= f64::EPSILON {
                    return Err(OpenCadError::validation("division by zero"));
                }
                value /= rhs;
                rest = next;
            }
            _ => break,
        }
    }
    Ok((value, rest))
}

fn parse_factor<'a>(
    tokens: &'a [Token],
    scope: &IndexMap<String, f64>,
) -> Result<(f64, &'a [Token])> {
    let (token, rest) = tokens
        .split_first()
        .ok_or_else(|| OpenCadError::InvalidExpression("empty expression".into()))?;
    match token {
        Token::Number(value) => Ok((*value, rest)),
        Token::Ident(name) => {
            if let Some(Token::Ident(unit)) = rest.first() {
                let value = scope
                    .get(name)
                    .copied()
                    .ok_or_else(|| OpenCadError::InvalidExpression(name.clone()))?;
                return Ok((convert_length(value / unit_factor("m"), unit), &rest[1..]));
            }
            let value = scope
                .get(name)
                .copied()
                .ok_or_else(|| OpenCadError::InvalidExpression(name.clone()))?;
            Ok((value, rest))
        }
        Token::Minus => {
            let (value, next) = parse_factor(rest, scope)?;
            Ok((-value, next))
        }
        _ => Err(OpenCadError::InvalidExpression("invalid factor".into())),
    }
}

fn convert_angle(value: f64, unit: &str) -> f64 {
    value * angle_unit_factor(unit)
}

fn angle_unit_factor(unit: &str) -> f64 {
    match unit {
        "deg" | "degree" | "degrees" => std::f64::consts::PI / 180.0,
        "rad" | "radian" | "radians" => 1.0,
        _ => 1.0,
    }
}

fn convert_length(value: f64, unit: &str) -> f64 {
    value * unit_factor(unit)
}

fn unit_factor(unit: &str) -> f64 {
    match unit {
        "m" => 1.0,
        "mm" => 0.001,
        "cm" => 0.01,
        "in" => 0.0254,
        _ => 1.0,
    }
}

/// Default bracket parameters for samples and fixtures.
pub fn bracket_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    graph
        .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
        .expect("width");
    graph
        .add_parameter(ParameterEntry::new("param:height", "height", "60 mm"))
        .expect("height");
    graph
        .add_parameter(ParameterEntry::new("param:thickness", "thickness", "6 mm"))
        .expect("thickness");
    graph
        .add_parameter(ParameterEntry::new(
            "param:hole_diameter",
            "hole_diameter",
            "10 mm",
        ))
        .expect("hole_diameter");
    graph
        .add_parameter(ParameterEntry::new(
            "param:fillet_radius",
            "fillet_radius",
            "1 mm",
        ))
        .expect("fillet_radius");
    graph
        .add_parameter(ParameterEntry::new(
            "param:chamfer_distance",
            "chamfer_distance",
            "0.5 mm",
        ))
        .expect("chamfer_distance");
    graph
        .add_parameter(ParameterEntry::new(
            "param:hole_pitch",
            "hole_pitch",
            "20 mm",
        ))
        .expect("hole_pitch");
    graph
        .add_parameter(ParameterEntry::new(
            "param:boss_height",
            "boss_height",
            "12 mm",
        ))
        .expect("boss_height");
    graph
}

/// Parameters for the multi-feature bearing carrier sample.
pub fn bearing_carrier_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expression) in [
        ("param:width", "width", "96 mm"),
        ("param:height", "height", "72 mm"),
        ("param:thickness", "thickness", "8 mm"),
        ("param:boss_outer_diameter", "boss_outer_diameter", "38 mm"),
        ("param:bore_diameter", "bore_diameter", "18 mm"),
        ("param:boss_height", "boss_height", "14 mm"),
        ("param:bolt_hole_diameter", "bolt_hole_diameter", "5.5 mm"),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expression))
            .expect("static bearing carrier parameter");
    }
    graph
}

/// Parameters for the robot-joint actuator housing flagship sample.
pub fn robot_joint_housing_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expression) in [
        ("param:width", "width", "140 mm"),
        ("param:height", "height", "110 mm"),
        ("param:base_thickness", "base_thickness", "10 mm"),
        ("param:lower_hub_diameter", "lower_hub_diameter", "72 mm"),
        ("param:lower_hub_height", "lower_hub_height", "20 mm"),
        ("param:upper_hub_diameter", "upper_hub_diameter", "52 mm"),
        ("param:upper_hub_height", "upper_hub_height", "32 mm"),
        ("param:shaft_diameter", "shaft_diameter", "24 mm"),
        (
            "param:counterbore_diameter",
            "counterbore_diameter",
            "38 mm",
        ),
        ("param:counterbore_depth", "counterbore_depth", "8 mm"),
        ("param:bolt_circle_radius", "bolt_circle_radius", "45 mm"),
        ("param:bolt_hole_diameter", "bolt_hole_diameter", "6.2 mm"),
        ("param:rib_length", "rib_length", "38 mm"),
        ("param:rib_thickness", "rib_thickness", "7 mm"),
        ("param:rib_height", "rib_height", "15 mm"),
        ("param:mounting_ear_width", "mounting_ear_width", "22 mm"),
        ("param:mounting_ear_depth", "mounting_ear_depth", "34 mm"),
        ("param:mounting_ear_height", "mounting_ear_height", "16 mm"),
        (
            "param:mounting_hole_diameter",
            "mounting_hole_diameter",
            "9 mm",
        ),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expression))
            .expect("static robot-joint parameter must be valid");
    }
    graph
}

/// Parameters for the robot-arm base pedestal part.
pub fn robot_arm_base_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expression) in [
        ("param:base_plate_diameter", "base_plate_diameter", "120 mm"),
        (
            "param:base_plate_thickness",
            "base_plate_thickness",
            "10 mm",
        ),
        ("param:turret_diameter", "turret_diameter", "44 mm"),
        ("param:turret_height", "turret_height", "30 mm"),
        ("param:bolt_circle_radius", "bolt_circle_radius", "50 mm"),
        ("param:bolt_hole_diameter", "bolt_hole_diameter", "8 mm"),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expression))
            .expect("static robot-arm base parameter must be valid");
    }
    graph
}

/// Parameters for the robot-arm upper link part.
pub fn robot_arm_upper_arm_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expression) in [
        ("param:upper_arm_length", "upper_arm_length", "160 mm"),
        ("param:upper_arm_width", "upper_arm_width", "26 mm"),
        ("param:upper_arm_thickness", "upper_arm_thickness", "14 mm"),
        (
            "param:shoulder_hub_diameter",
            "shoulder_hub_diameter",
            "36 mm",
        ),
        (
            "param:shoulder_bore_diameter",
            "shoulder_bore_diameter",
            "14 mm",
        ),
        (
            "param:upper_arm_elbow_hub_diameter",
            "upper_arm_elbow_hub_diameter",
            "32 mm",
        ),
        (
            "param:upper_arm_elbow_bore_diameter",
            "upper_arm_elbow_bore_diameter",
            "12 mm",
        ),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expression))
            .expect("static robot-arm upper-arm parameter must be valid");
    }
    graph
}

/// Parameters for the robot-arm forearm link part.
pub fn robot_arm_forearm_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expression) in [
        ("param:forearm_length", "forearm_length", "110 mm"),
        ("param:forearm_width", "forearm_width", "24 mm"),
        ("param:forearm_thickness", "forearm_thickness", "12 mm"),
        (
            "param:forearm_elbow_hub_diameter",
            "forearm_elbow_hub_diameter",
            "32 mm",
        ),
        (
            "param:forearm_elbow_bore_diameter",
            "forearm_elbow_bore_diameter",
            "12 mm",
        ),
        ("param:wrist_hub_diameter", "wrist_hub_diameter", "30 mm"),
        ("param:wrist_bore_diameter", "wrist_bore_diameter", "10 mm"),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expression))
            .expect("static robot-arm forearm parameter must be valid");
    }
    graph
}

/// Parameters for the robot-arm wrist gripper part.
pub fn robot_arm_gripper_parameters() -> ParamGraph {
    let mut graph = ParamGraph::new();
    for (id, name, expression) in [
        ("param:gripper_length", "gripper_length", "40 mm"),
        ("param:gripper_width", "gripper_width", "36 mm"),
        ("param:gripper_thickness", "gripper_thickness", "14 mm"),
        (
            "param:gripper_wrist_bore_diameter",
            "gripper_wrist_bore_diameter",
            "10 mm",
        ),
        ("param:finger_diameter", "finger_diameter", "12 mm"),
        ("param:finger_spacing", "finger_spacing", "16 mm"),
        ("param:finger_length", "finger_length", "18 mm"),
    ] {
        graph
            .add_parameter(ParameterEntry::new(id, name, expression))
            .expect("static robot-arm gripper parameter must be valid");
    }
    graph
}

/// Default revolve bushing/sector parameters (lengths in mm, angle in degrees).
pub fn revolve_parameters(angle_expr: &str) -> ParamGraph {
    let mut graph = ParamGraph::new();
    graph
        .add_parameter(ParameterEntry::new(
            "param:inner_radius",
            "inner_radius",
            "15 mm",
        ))
        .expect("inner_radius");
    graph
        .add_parameter(ParameterEntry::new(
            "param:outer_radius",
            "outer_radius",
            "25 mm",
        ))
        .expect("outer_radius");
    graph
        .add_parameter(ParameterEntry::new("param:height", "height", "20 mm"))
        .expect("height");
    graph
        .add_parameter(ParameterEntry::new(
            "param:revolve_angle",
            "revolve_angle_rad",
            angle_expr,
        ))
        .expect("revolve_angle");
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_angle_degrees_to_radians() {
        let values = IndexMap::new();
        let radians = eval_angle_expr("180 deg", &values).expect("eval");
        assert!((radians - std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn evaluates_simple_units() {
        let values = IndexMap::new();
        assert!((eval_length_expr("80 mm", &values).expect("eval") - 0.08).abs() < 1e-9);
    }

    #[test]
    fn bearing_carrier_parameters_are_deterministic_and_unit_bearing() {
        let graph = bearing_carrier_parameters();
        let values = evaluate_param_graph(&graph).expect("evaluate carrier parameters");
        assert_eq!(graph.parameter_ids().len(), 7);
        assert!((values["boss_height"] - 0.014).abs() < 1e-12);
        assert!((values["bolt_hole_diameter"] - 0.0055).abs() < 1e-12);
    }

    #[test]
    fn robot_joint_housing_parameters_are_deterministic_and_unit_bearing() {
        let graph = robot_joint_housing_parameters();
        let values = evaluate_param_graph(&graph).expect("evaluate housing parameters");
        assert_eq!(graph.parameter_ids().len(), 19);
        assert!((values["upper_hub_height"] - 0.032).abs() < 1e-12);
        assert!((values["bolt_circle_radius"] - 0.045).abs() < 1e-12);
        assert!((values["mounting_hole_diameter"] - 0.009).abs() < 1e-12);
        assert_eq!(
            graph.evaluation_order().expect("order"),
            graph.parameter_ids()
        );
    }

    #[test]
    fn evaluates_param_dependencies() {
        let mut graph = ParamGraph::new();
        graph
            .add_parameter(ParameterEntry::new("param:width", "width", "80 mm"))
            .expect("width");
        graph
            .add_parameter(ParameterEntry::new("param:half", "half", "width / 2"))
            .expect("half");
        graph
            .add_dependency("param:width", "param:half")
            .expect("dep");

        let values = evaluate_param_graph(&graph).expect("eval");
        assert!((values["width"] - 0.08).abs() < 1e-9);
        assert!((values["half"] - 0.04).abs() < 1e-9);
    }

    #[test]
    fn parameter_names_in_expr_extracts_identifiers() {
        let names = parameter_names_in_expr("hole_diameter / 2");
        assert_eq!(names, vec!["hole_diameter".to_string()]);
    }

    #[test]
    fn robot_arm_parameters_are_deterministic_and_unit_bearing() {
        let base = evaluate_param_graph(&robot_arm_base_parameters()).expect("base params");
        assert_eq!(robot_arm_base_parameters().parameter_ids().len(), 6);
        assert!((base["base_plate_diameter"] - 0.12).abs() < 1e-12);
        assert!((base["turret_height"] - 0.03).abs() < 1e-12);
        assert!((base["bolt_circle_radius"] - 0.05).abs() < 1e-12);

        let upper = evaluate_param_graph(&robot_arm_upper_arm_parameters()).expect("upper params");
        assert_eq!(robot_arm_upper_arm_parameters().parameter_ids().len(), 7);
        assert!((upper["upper_arm_length"] - 0.16).abs() < 1e-12);
        assert!((upper["upper_arm_thickness"] - 0.014).abs() < 1e-12);

        let forearm =
            evaluate_param_graph(&robot_arm_forearm_parameters()).expect("forearm params");
        assert_eq!(robot_arm_forearm_parameters().parameter_ids().len(), 7);
        assert!((forearm["forearm_length"] - 0.11).abs() < 1e-12);

        let gripper =
            evaluate_param_graph(&robot_arm_gripper_parameters()).expect("gripper params");
        assert_eq!(robot_arm_gripper_parameters().parameter_ids().len(), 7);
        assert!((gripper["finger_spacing"] - 0.016).abs() < 1e-12);
    }
}
