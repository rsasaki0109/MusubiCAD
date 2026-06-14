//! Parameter expression evaluation.

use indexmap::IndexMap;

use opencad_core::{OpenCadError, Result};

use crate::param_graph::{ParamGraph, ParameterEntry};

/// Evaluate all parameters in dependency order. Values are meters.
pub fn evaluate_param_graph(graph: &ParamGraph) -> Result<IndexMap<String, f64>> {
    let order = graph.evaluation_order()?;
    let mut values = IndexMap::new();
    for id in order {
        let entry = graph
            .get(&id)
            .ok_or_else(|| OpenCadError::not_found(format!("parameter '{id}'")))?;
        let value = eval_length_expr(&entry.expr, &values)?;
        values.insert(entry.name.clone(), value);
    }
    Ok(values)
}

/// Evaluate a length expression in meters using resolved parameter names.
pub fn eval_length_expr(expr: &str, scope: &IndexMap<String, f64>) -> Result<f64> {
    let tokens = tokenize(expr)?;
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

fn tokenize(input: &str) -> Result<Vec<Token>> {
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
                tokens.push(Token::Number(convert_length(value, &unit)));
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
}

/// Default revolve bushing/sector parameters (lengths in mm, angle in radians).
pub fn revolve_parameters(angle_rad_expr: &str) -> ParamGraph {
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
            angle_rad_expr,
        ))
        .expect("revolve_angle");
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_simple_units() {
        let values = IndexMap::new();
        assert!((eval_length_expr("80 mm", &values).expect("eval") - 0.08).abs() < 1e-9);
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
}
