//! ADR-074 diagnostic reference input; not a model or backend mass query.
use opencad_core::Length;
use opencad_geometry::CircularFilletReference;
use serde_json::json;

fn main() -> opencad_core::Result<()> {
    let mut references = serde_json::Map::new();
    for (name, minimum, maximum) in [("constant", 0.001, 0.001), ("variable", 0.0005, 0.0015)] {
        let reference = CircularFilletReference::new(
            Length::from_meters(0.01),
            Length::from_meters(0.02),
            Length::from_meters(minimum),
            Length::from_meters(maximum),
        )?;
        let bounds = reference.volume_bounds_m3(1e-12, 262_144)?;
        references.insert(
            name.into(),
            json!({
                "minimum_radius_m": minimum, "maximum_radius_m": maximum,
                "body_lower_m3": bounds.body_lower_m3,
                "body_upper_m3": bounds.body_upper_m3, "cells": bounds.cells
            }),
        );
    }
    println!(
        "{}",
        json!({"format_version":1,"radius_m":0.01,"height_m":0.02,
        "absolute_width_m3":1e-12,"references":references})
    );
    Ok(())
}
