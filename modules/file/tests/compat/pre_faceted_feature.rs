// Exact pre-extension enum from 78f62e25eb43a413ea2dbb15097dea3b3cdfa14b:
// modules/feature/src/feature.rs. Only type name/import paths are adapted.
// Serde's real old tagged parser must reject the new feature, not ignore fields.
use opencad_feature::*;
#[allow(dead_code)]
#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreviousFeatureDefinition {
    DatumPlane(opencad_feature::datum::DatumPlaneFeature),
    DatumAxis(opencad_feature::datum::DatumAxisFeature),
    Sketch(SketchFeatureDef),
    ProjectedSketch(opencad_feature::ProjectedSketchFeature),
    Extrude(ExtrudeFeature),
    Revolve(RevolveFeature),
    Hole(HoleFeature),
    StandardHole(opencad_feature::standard_hole::StandardHoleFeature),
    SeatedStandardHole(opencad_feature::seated_standard_hole::SeatedStandardHoleFeature),
    Fillet(FilletFeature),
    Chamfer(ChamferFeature),
    LinearPattern(LinearPatternFeature),
    CircularPattern(CircularPatternFeature),
    MirrorPattern(MirrorPatternFeature),
    ImportedSolid(ImportedSolidFeature),
    ImportedMesh(opencad_feature::ImportedMeshFeature),
    Shell(ShellFeature),
    Draft(opencad_feature::draft::DraftFeature),
    CylindricalDraft(opencad_feature::cylindrical_draft::CylindricalDraftFeature),
    NominalMetricThread(opencad_feature::nominal_metric_thread::NominalMetricThreadFeature),
    Rib(RibFeature),
    TaperedRib(opencad_feature::tapered_rib::TaperedRibFeature),
    BodyBoolean(opencad_feature::body_boolean::BodyBooleanFeature),
    Loft(LoftFeature),
    Sweep(SweepFeature),
    HelixSweep(HelixSweepFeature),
}
