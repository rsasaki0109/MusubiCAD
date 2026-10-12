//! Bounded, explicit-unit planar DXF import into authored sketches (ADR-048).
use opencad_core::{EntityId, LengthUnit, OpenCadError, Result, SketchId};
use opencad_sketch::{
    ArcEntity, CircleEntity, Coord, EntityBase, LineEntity, PointEntity, Sketch, SketchEntity,
    Workplane,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DXF_MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAIRS: usize = 500_000;
const MAX_PRIMITIVES: usize = 50_000;
const GRID_M: f64 = 1e-9;
type Tag<'a> = (i32, &'a str);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfInspection {
    pub version: Option<String>,
    pub declared_unit_code: Option<i32>,
    pub declared_unit: Option<LengthUnit>,
    pub entities_by_layer: BTreeMap<String, BTreeMap<String, usize>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DxfSketch {
    pub sketch: Sketch,
    pub source_unit: LengthUnit,
    pub selected_layers: Vec<String>,
    pub primitive_count: usize,
}
struct Parsed<'a> {
    inspection: DxfInspection,
    entities: Vec<Vec<Tag<'a>>>,
}

pub fn inspect_dxf(text: &str) -> Result<DxfInspection> {
    Ok(parse(text)?.inspection)
}

pub fn import_dxf_sketch(
    text: &str,
    id: SketchId,
    name: &str,
    layers: &[String],
    unit: Option<LengthUnit>,
) -> Result<DxfSketch> {
    let parsed = parse(text)?;
    let source_unit = match (
        parsed.inspection.declared_unit_code,
        parsed.inspection.declared_unit,
        unit,
    ) {
        (_, Some(declared), Some(given)) if declared != given => {
            return Err(invalid("source unit conflicts with INSUNITS"))
        }
        (_, Some(declared), _) => declared,
        (None | Some(0), None, Some(given)) => given,
        (None | Some(0), None, None) => {
            return Err(invalid(
                "missing/unassigned units require explicit m/mm/cm/in",
            ))
        }
        _ => return Err(invalid("unsupported declared INSUNITS code")),
    };
    let selected: BTreeSet<_> = layers.iter().map(String::as_str).collect();
    if selected.is_empty()
        || selected.len() != layers.len()
        || selected
            .iter()
            .any(|l| !parsed.inspection.entities_by_layer.contains_key(*l))
    {
        return Err(invalid("select nonempty, unique, existing geometry layers"));
    }
    if id.as_str().len() > 128 {
        return Err(invalid("sketch ID exceeds 128 characters"));
    }
    let scale = source_unit.to_meters_factor();
    let mut builder = Builder {
        prefix: format!("dxf_{}", id.as_str().trim_start_matches("sketch:")),
        sketch: Sketch::new(id, name, Workplane::xy()),
        points: BTreeMap::new(),
        primitives: 0,
    };
    for e in &parsed.entities {
        if !selected.contains(layer(e)?) {
            continue;
        }
        planar(e, scale)?;
        match e[0].1 {
            "LINE" => builder.line(point(e, 10, scale)?, point(e, 11, scale)?)?,
            "CIRCLE" | "ARC" => {
                let center = point(e, 10, scale)?;
                let radius = number(e, 40, None)? * scale;
                if !radius.is_finite() || radius <= GRID_M || radius > 1e6 {
                    return Err(invalid("invalid radius in source units"));
                }
                if e[0].1 == "CIRCLE" {
                    builder.circle(center, radius)?;
                } else {
                    let start = number(e, 50, None)?.rem_euclid(360.).to_radians();
                    let end = number(e, 51, None)?.rem_euclid(360.).to_radians();
                    let span = (end - start).rem_euclid(std::f64::consts::TAU);
                    if !span.is_finite() || span <= 1e-12 {
                        return Err(invalid(
                            "degenerate/full ARC; use CIRCLE for complete circles",
                        ));
                    }
                    builder.arc(center, radius, start, start + span)?;
                }
            }
            "LWPOLYLINE" => {
                let flags = integer(e, 70, Some(0))?;
                if flags < 0 || flags & !129 != 0 {
                    return Err(invalid("unsupported polyline flags"));
                }
                let count = integer(e, 90, None)?;
                let mut points = Vec::new();
                let mut x = None;
                for &(code, value) in e {
                    match code {
                        10 => {
                            if x.is_some() {
                                return Err(invalid("polyline vertex missing y"));
                            }
                            x = Some(finite(value)? * scale);
                        }
                        20 => points.push([
                            x.take().ok_or_else(|| invalid("polyline y without x"))?,
                            finite(value)? * scale,
                        ]),
                        40 | 41 | 43 if (finite(value)? * scale).abs() > GRID_M => {
                            return Err(invalid("polyline widths unsupported"))
                        }
                        42 if finite(value)?.abs() > 1e-15 => {
                            return Err(invalid("curved polyline bulges unsupported"))
                        }
                        _ => {}
                    }
                }
                if x.is_some() || count < 2 || count as usize != points.len() {
                    return Err(invalid("polyline vertex count differs"));
                }
                for pair in points.windows(2) {
                    builder.line(pair[0], pair[1])?;
                }
                if flags & 1 != 0 {
                    builder.line(
                        *points.last().ok_or_else(|| invalid("empty polyline"))?,
                        points[0],
                    )?;
                }
            }
            kind => {
                return Err(invalid(format!(
                    "unsupported selected entity '{kind}' on layer '{}'",
                    layer(e)?
                )))
            }
        }
    }
    if builder.primitives == 0 {
        return Err(invalid("selected layers contain no supported geometry"));
    }
    Ok(DxfSketch {
        source_unit,
        selected_layers: selected.into_iter().map(String::from).collect(),
        primitive_count: builder.primitives,
        sketch: builder.sketch,
    })
}

fn parse(text: &str) -> Result<Parsed<'_>> {
    if text.len() > DXF_MAX_BYTES {
        return Err(invalid("input exceeds 16 MiB"));
    }
    let lines: Vec<_> = text
        .trim_start_matches('\u{feff}')
        .trim_end()
        .lines()
        .collect();
    if lines.len() % 2 != 0 || lines.len() / 2 > MAX_PAIRS {
        return Err(invalid("unpaired/excessive DXF tags"));
    }
    let tags: Vec<Tag<'_>> = lines
        .chunks_exact(2)
        .map(|p| {
            let code: i32 = p[0]
                .trim()
                .parse()
                .map_err(|_| invalid("invalid group code"))?;
            if !(0..=1071).contains(&code) {
                return Err(invalid("unsupported group code range"));
            }
            Ok((code, p[1].trim()))
        })
        .collect::<Result<_>>()?;
    let mut inspection = DxfInspection {
        version: None,
        declared_unit_code: None,
        declared_unit: None,
        entities_by_layer: BTreeMap::new(),
    };
    let mut ranges = Vec::new();
    let mut section = None;
    let mut seen = BTreeSet::new();
    let mut begin = None;
    let mut ended = false;
    let mut i = 0;
    while i < tags.len() {
        let (code, value) = tags[i];
        if code == 0 {
            if section == Some("ENTITIES") {
                if let Some(start) = begin.take() {
                    ranges.push(start..i);
                }
            }
            match value {
                "SECTION" => {
                    if section.is_some()
                        || tags.get(i + 1).map(|p| p.0) != Some(2)
                        || !seen.insert(tags[i + 1].1)
                    {
                        return Err(invalid("malformed/nested/duplicate section"));
                    }
                    section = Some(tags[i + 1].1);
                    i += 1;
                }
                "ENDSEC" => {
                    if section.take().is_none() {
                        return Err(invalid("unexpected ENDSEC"));
                    }
                }
                "EOF" => {
                    if section.is_some() || i + 1 != tags.len() {
                        return Err(invalid("malformed EOF"));
                    }
                    ended = true;
                }
                _ if section == Some("ENTITIES") => begin = Some(i),
                _ if section.is_none() => return Err(invalid("record outside section")),
                _ => {}
            }
        } else if section == Some("HEADER") && code == 9 {
            let next = tags
                .get(i + 1)
                .ok_or_else(|| invalid("header value missing"))?;
            match value {
                "$ACADVER" => {
                    if inspection.version.is_some() || next.0 != 1 {
                        return Err(invalid("ambiguous version"));
                    }
                    inspection.version = Some(next.1.into());
                }
                "$INSUNITS" => {
                    if inspection.declared_unit_code.is_some() || next.0 != 70 {
                        return Err(invalid("ambiguous INSUNITS"));
                    }
                    let unit: i32 = next.1.parse().map_err(|_| invalid("invalid INSUNITS"))?;
                    inspection.declared_unit_code = Some(unit);
                    inspection.declared_unit = match unit {
                        1 => Some(LengthUnit::Inch),
                        4 => Some(LengthUnit::Millimeter),
                        5 => Some(LengthUnit::Centimeter),
                        6 => Some(LengthUnit::Meter),
                        _ => None,
                    };
                }
                _ => {}
            }
        } else if section == Some("ENTITIES") && begin.is_none() {
            return Err(invalid("entity tags before record"));
        }
        i += 1;
    }
    if !ended || section.is_some() {
        return Err(invalid("unterminated DXF"));
    }
    if inspection.version.as_deref().is_some_and(|v| {
        !matches!(
            v,
            "AC1009"
                | "AC1012"
                | "AC1014"
                | "AC1015"
                | "AC1018"
                | "AC1021"
                | "AC1024"
                | "AC1027"
                | "AC1032"
        )
    }) {
        return Err(invalid("unsupported DXF version"));
    }
    let entities: Vec<Vec<Tag<'_>>> = ranges.into_iter().map(|r| tags[r].to_vec()).collect();
    for entity in &entities {
        let counts = inspection
            .entities_by_layer
            .entry(layer(entity)?.into())
            .or_default();
        *counts.entry(entity[0].1.into()).or_default() += 1;
    }
    Ok(Parsed {
        inspection,
        entities,
    })
}

fn single<'a>(e: &[Tag<'a>], code: i32) -> Result<Option<&'a str>> {
    let mut values = e.iter().filter(|p| p.0 == code);
    let first = values.next().map(|p| p.1);
    if values.next().is_some() {
        return Err(invalid(format!("ambiguous geometry tag {code}")));
    }
    Ok(first)
}
fn finite(value: &str) -> Result<f64> {
    let n: f64 = value
        .parse()
        .map_err(|_| invalid("invalid numeric value"))?;
    if !n.is_finite() {
        return Err(invalid("nonfinite numeric value"));
    }
    Ok(n)
}
fn number(e: &[Tag<'_>], code: i32, default: Option<f64>) -> Result<f64> {
    match single(e, code)? {
        Some(s) => finite(s),
        None => default.ok_or_else(|| invalid(format!("missing geometry tag {code}"))),
    }
}
fn integer(e: &[Tag<'_>], code: i32, default: Option<i32>) -> Result<i32> {
    match single(e, code)? {
        Some(s) => s.parse().map_err(|_| invalid("invalid integer tag")),
        None => default.ok_or_else(|| invalid("missing integer tag")),
    }
}
fn layer<'a>(e: &[Tag<'a>]) -> Result<&'a str> {
    let value = single(e, 8)?.unwrap_or("0");
    if value.is_empty() {
        return Err(invalid("empty layer name"));
    }
    Ok(value)
}
fn point(e: &[Tag<'_>], code: i32, scale: f64) -> Result<[f64; 2]> {
    if (number(e, code + 20, Some(0.))? * scale).abs() > GRID_M {
        return Err(invalid("nonplanar geometry"));
    }
    Ok([
        number(e, code, None)? * scale,
        number(e, code + 10, None)? * scale,
    ])
}
fn planar(e: &[Tag<'_>], scale: f64) -> Result<()> {
    if integer(e, 67, Some(0))? != 0
        || (number(e, 39, Some(0.))? * scale).abs() > GRID_M
        || (number(e, 38, Some(0.))? * scale).abs() > GRID_M
    {
        return Err(invalid("paper-space/elevated/thick geometry unsupported"));
    }
    for (code, wanted) in [(210, 0.), (220, 0.), (230, 1.)] {
        if (number(e, code, Some(wanted))? - wanted).abs() > 1e-12 {
            return Err(invalid("non-default OCS unsupported"));
        }
    }
    Ok(())
}

struct Builder {
    prefix: String,
    sketch: Sketch,
    points: BTreeMap<[i64; 2], EntityId>,
    primitives: usize,
}
impl Builder {
    fn point(&mut self, p: [f64; 2]) -> Result<EntityId> {
        if !p.iter().all(|v| v.is_finite() && v.abs() <= 1e6) {
            return Err(invalid("coordinate exceeds finite ±1e6 m bound"));
        }
        let key = p.map(|v| (v / GRID_M).round() as i64);
        if let Some(id) = self.points.get(&key) {
            return Ok(id.clone());
        }
        let id = EntityId::new(format!("ent:{}_p{}", self.prefix, self.points.len()))?;
        self.sketch.add_entity(SketchEntity::Point(PointEntity {
            base: EntityBase {
                id: id.clone(),
                construction: false,
            },
            x: Coord::literal(key[0] as f64 * GRID_M),
            y: Coord::literal(key[1] as f64 * GRID_M),
        }))?;
        self.points.insert(key, id.clone());
        Ok(id)
    }
    fn base(&mut self) -> Result<EntityBase> {
        if self.primitives >= MAX_PRIMITIVES {
            return Err(invalid("selected geometry exceeds 50000 primitives"));
        }
        let id = EntityId::new(format!("ent:{}_e{}", self.prefix, self.primitives))?;
        self.primitives += 1;
        Ok(EntityBase {
            id,
            construction: false,
        })
    }
    fn line(&mut self, a: [f64; 2], b: [f64; 2]) -> Result<()> {
        let start = self.point(a)?;
        let end = self.point(b)?;
        if start == end {
            return Err(invalid("segment collapses on 1 nm grid"));
        }
        let base = self.base()?;
        self.sketch
            .add_entity(SketchEntity::Line(LineEntity { base, start, end }))
    }
    fn circle(&mut self, c: [f64; 2], r: f64) -> Result<()> {
        let center = self.point(c)?;
        let base = self.base()?;
        self.sketch.add_entity(SketchEntity::Circle(CircleEntity {
            base,
            center,
            radius: Coord::literal(r),
        }))
    }
    fn arc(&mut self, c: [f64; 2], r: f64, start: f64, end: f64) -> Result<()> {
        let center = self.point(c)?;
        let a = self.point([c[0] + r * start.cos(), c[1] + r * start.sin()])?;
        let b = self.point([c[0] + r * end.cos(), c[1] + r * end.sin()])?;
        if a == b {
            return Err(invalid("arc endpoints collapse on 1 nm grid"));
        }
        let base = self.base()?;
        self.sketch.add_entity(SketchEntity::Arc(ArcEntity {
            base,
            center,
            radius: Coord::literal(r),
            start_angle: Coord::literal(start),
            end_angle: Coord::literal(end),
            start_point: Some(a),
            end_point: Some(b),
            resolved_angles_rad: None,
        }))
    }
}
fn invalid(message: impl Into<String>) -> OpenCadError {
    OpenCadError::validation(format!("DXF: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(entity: &str, unit: Option<i32>) -> String {
        let units = unit
            .map(|u| format!("9\n$INSUNITS\n70\n{u}\n"))
            .unwrap_or_default();
        format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1027\n{units}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entity}0\nENDSEC\n0\nEOF\n")
    }
    const RECT:&str="0\nLWPOLYLINE\n8\nOUTLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n80\n20\n0\n10\n80\n20\n60\n10\n0\n20\n60\n";
    fn imported(t: &str, unit: Option<LengthUnit>) -> Result<DxfSketch> {
        import_dxf_sketch(
            t,
            SketchId::new("sketch:import")?,
            "Import",
            &["OUTLINE".into()],
            unit,
        )
    }
    #[test]
    fn analytic_si_rectangle_shared_points_layer_selection_and_round_trip() {
        let t = text(
            &(RECT.to_string()
                + "0\nTEXT\n8\nLABELS\n1\nignored only by explicit layer selection\n"),
            Some(4),
        );
        let inspection = inspect_dxf(&t).expect("inspect");
        assert_eq!(inspection.declared_unit, Some(LengthUnit::Millimeter));
        assert_eq!(inspection.entities_by_layer["OUTLINE"]["LWPOLYLINE"], 1);
        let mut result = imported(&t, None).expect("import");
        assert_eq!(result.primitive_count, 4);
        assert_eq!(result.sketch.entities.len(), 8);
        let points: Vec<_> = result
            .sketch
            .entities
            .iter()
            .filter_map(|e| match e {
                SketchEntity::Point(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(points.len(), 4);
        for (actual, expected) in
            points
                .iter()
                .zip([[0., 0.], [0.08, 0.], [0.08, 0.06], [0., 0.06]])
        {
            for (coord, wanted) in [&actual.x, &actual.y].into_iter().zip(expected) {
                let Coord::Literal(v) = coord else {
                    panic!("literal");
                };
                assert!((*v - wanted).abs() < 1e-12);
            }
        }
        result.sketch.update_profiles().expect("closed profile");
        assert_eq!(result.sketch.profiles.len(), 1);
        assert_eq!(
            result,
            serde_json::from_str::<DxfSketch>(&serde_json::to_string(&result).expect("json"))
                .expect("round trip")
        );
        assert!(import_dxf_sketch(
            &t,
            SketchId::new("sketch:import").expect("id"),
            "Bad",
            &["OUTLINE".into(), "LABELS".into()],
            None
        )
        .is_err());
    }
    #[test]
    fn units_circle_and_wrapping_arc_remain_analytic() {
        for (code, scale) in [(1, 0.0254), (4, 0.001), (5, 0.01), (6, 1.)] {
            let t = text("0\nCIRCLE\n8\nOUTLINE\n10\n10\n20\n20\n40\n2\n", Some(code));
            let result = imported(&t, None).expect("circle");
            let SketchEntity::Circle(c) = &result.sketch.entities[1] else {
                panic!("circle");
            };
            let Coord::Literal(radius) = c.radius else {
                panic!("radius");
            };
            assert!((radius - 2. * scale).abs() < 1e-12);
        }
        let t = text(
            "0\nARC\n8\nOUTLINE\n10\n0\n20\n0\n40\n10\n50\n350\n51\n10\n",
            Some(4),
        );
        let result = imported(&t, None).expect("arc");
        let SketchEntity::Arc(a) = result.sketch.entities.last().expect("arc") else {
            panic!("arc");
        };
        let (Coord::Literal(start), Coord::Literal(end)) = (&a.start_angle, &a.end_angle) else {
            panic!("angles");
        };
        assert!((end - start - 20_f64.to_radians()).abs() < 1e-12);
        assert!(a.start_point.is_some() && a.end_point.is_some());
        assert!(imported(&text(RECT, None), None).is_err());
        assert!(imported(&text(RECT, None), Some(LengthUnit::Millimeter)).is_ok());
        assert!(imported(&text(RECT, Some(4)), Some(LengthUnit::Meter)).is_err());
        assert!(imported(&text(RECT, Some(2)), Some(LengthUnit::Inch)).is_err());
    }
    #[test]
    fn malformed_ambiguous_nonplanar_curved_and_collapsed_inputs_fail_closed() {
        for e in [
            "0\nSPLINE\n8\nOUTLINE\n",
            "0\nLINE\n8\nOUTLINE\n10\n0\n20\n0\n30\n1\n11\n80\n21\n0\n",
            "0\nLINE\n8\nOUTLINE\n10\n0\n10\n1\n20\n0\n11\n80\n21\n0\n",
            "0\nLINE\n8\nOUTLINE\n10\n0\n20\nNaN\n11\n80\n21\n0\n",
            "0\nLINE\n8\nOUTLINE\n10\n0\n20\n0\n11\n0.0000001\n21\n0\n",
            "0\nCIRCLE\n8\nOUTLINE\n10\n0\n20\n0\n40\n10\n230\n-1\n",
            "0\nLWPOLYLINE\n8\nOUTLINE\n90\n2\n10\n0\n20\n0\n42\n0.1\n10\n80\n20\n0\n",
            "0\nLWPOLYLINE\n8\nOUTLINE\n90\n2\n43\n1\n10\n0\n20\n0\n10\n80\n20\n0\n",
        ] {
            assert!(imported(&text(e, Some(4)), None).is_err(), "accepted {e}");
        }
        assert!(inspect_dxf("0\nSECTION\n2").is_err());
        assert!(inspect_dxf(&text(RECT, Some(4)).replace("0\nEOF\n", "")).is_err());
        assert!(inspect_dxf(&text(RECT, Some(4)).replace(
            "9\n$INSUNITS\n70\n4\n",
            "9\n$INSUNITS\n70\n4\n9\n$INSUNITS\n70\n4\n"
        ))
        .is_err());
        assert!(inspect_dxf(&"x".repeat(DXF_MAX_BYTES + 1)).is_err());
    }
}
