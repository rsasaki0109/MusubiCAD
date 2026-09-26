//! `opencad new` command (Task-121+).

use opencad_assembly::{
    robot_arm_assembly_model, AssemblyModel, Component, Instance, Mate, MateEntity, MateKind,
    Placement,
};
use opencad_core::{DocumentId, DocumentMetadata, Result};
use opencad_feature::{
    bearing_carrier, bracket_boss_join, bracket_edge_fillet, bracket_face_pin, bracket_hole_ring,
    bracket_hole_row, bracket_pin_mirror, bracket_pin_ring, bracket_pin_row, bracket_semantic_refs,
    bracket_with_hole, revolve_bushing, revolve_sector, robot_arm_base, robot_arm_forearm,
    robot_arm_gripper, robot_arm_upper_arm, robot_joint_actuator_housing,
};
use opencad_file::{write_ocad, OcadDocument};
use opencad_geometry::RigidTransform;
use opencad_graph::{bracket_parameters, revolve_parameters, FeatureGraph, ParamGraph};

/// Built-in sample document templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentTemplate {
    #[default]
    Bracket,
    BearingCarrier,
    RobotJoint,
    BossJoin,
    FacePin,
    EdgeFillet,
    HoleRow,
    HoleRing,
    PinRow,
    PinRing,
    PinMirror,
    RevolveBushing,
    RevolveSector,
    AssemblyTwoBrackets,
    RobotArmAssembly,
    BracketFrontViewDrawing,
}

impl DocumentTemplate {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "bracket" => Ok(Self::Bracket),
            "bearing-carrier" => Ok(Self::BearingCarrier),
            "robot-joint" => Ok(Self::RobotJoint),
            "boss-join" => Ok(Self::BossJoin),
            "face-pin" => Ok(Self::FacePin),
            "edge-fillet" => Ok(Self::EdgeFillet),
            "hole-row" => Ok(Self::HoleRow),
            "hole-ring" => Ok(Self::HoleRing),
            "pin-row" => Ok(Self::PinRow),
            "pin-ring" => Ok(Self::PinRing),
            "pin-mirror" => Ok(Self::PinMirror),
            "revolve-bushing" => Ok(Self::RevolveBushing),
            "revolve-sector" => Ok(Self::RevolveSector),
            "assembly" => Ok(Self::AssemblyTwoBrackets),
            "robot-arm" => Ok(Self::RobotArmAssembly),
            "drawing" => Ok(Self::BracketFrontViewDrawing),
            _ => Err(opencad_core::OpenCadError::validation(format!(
                "unknown template '{name}'; expected 'bracket', 'bearing-carrier', 'robot-joint', 'boss-join', 'face-pin', 'edge-fillet', 'hole-row', 'hole-ring', 'pin-row', 'pin-ring', 'pin-mirror', 'revolve-bushing', 'revolve-sector', 'assembly', 'robot-arm', or 'drawing'"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bracket => "bracket",
            Self::BearingCarrier => "bearing-carrier",
            Self::RobotJoint => "robot-joint",
            Self::BossJoin => "boss-join",
            Self::FacePin => "face-pin",
            Self::EdgeFillet => "edge-fillet",
            Self::HoleRow => "hole-row",
            Self::HoleRing => "hole-ring",
            Self::PinRow => "pin-row",
            Self::PinRing => "pin-ring",
            Self::PinMirror => "pin-mirror",
            Self::RevolveBushing => "revolve-bushing",
            Self::RevolveSector => "revolve-sector",
            Self::AssemblyTwoBrackets => "assembly",
            Self::RobotArmAssembly => "robot-arm",
            Self::BracketFrontViewDrawing => "drawing",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Bracket,
            Self::BearingCarrier,
            Self::RobotJoint,
            Self::BossJoin,
            Self::FacePin,
            Self::EdgeFillet,
            Self::HoleRow,
            Self::HoleRing,
            Self::PinRow,
            Self::PinRing,
            Self::PinMirror,
            Self::RevolveBushing,
            Self::RevolveSector,
            Self::AssemblyTwoBrackets,
            Self::RobotArmAssembly,
            Self::BracketFrontViewDrawing,
        ]
    }
}

pub fn create_document(path: &str, template: DocumentTemplate) -> Result<()> {
    match template {
        DocumentTemplate::Bracket => create_bracket_document(path),
        DocumentTemplate::BearingCarrier => create_bearing_carrier_document(path),
        DocumentTemplate::RobotJoint => create_robot_joint_document(path),
        DocumentTemplate::BossJoin => create_bracket_boss_join_document(path),
        DocumentTemplate::FacePin => create_bracket_face_pin_document(path),
        DocumentTemplate::EdgeFillet => create_bracket_edge_fillet_document(path),
        DocumentTemplate::HoleRow => create_bracket_hole_row_document(path),
        DocumentTemplate::HoleRing => create_bracket_hole_ring_document(path),
        DocumentTemplate::PinRow => create_bracket_pin_row_document(path),
        DocumentTemplate::PinRing => create_bracket_pin_ring_document(path),
        DocumentTemplate::PinMirror => create_bracket_pin_mirror_document(path),
        DocumentTemplate::RevolveBushing => create_revolve_bushing_document(path),
        DocumentTemplate::RevolveSector => create_revolve_sector_document(path),
        DocumentTemplate::AssemblyTwoBrackets => create_assembly_two_brackets_document(path),
        DocumentTemplate::RobotArmAssembly => create_robot_arm_assembly_document(path),
        DocumentTemplate::BracketFrontViewDrawing => create_bracket_front_view_document(path),
    }
}

pub fn create_robot_joint_document(path: &str) -> Result<()> {
    let part = robot_joint_actuator_housing()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:robot_joint_actuator_001")?,
        "Robot Joint Actuator Housing",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = opencad_graph::robot_joint_housing_parameters();
    write_ocad(path, &doc)
}

pub fn create_bearing_carrier_document(path: &str) -> Result<()> {
    let part = bearing_carrier()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bearing_carrier_001")?,
        "Flanged Bearing Carrier",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = opencad_graph::bearing_carrier_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_document(path: &str) -> Result<()> {
    let part = bracket_with_hole()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_001")?,
        "Bracket with Mounting Hole",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_bracket_boss_join_document(path: &str) -> Result<()> {
    let part = bracket_boss_join()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_boss_join_001")?,
        "Bracket with Joined Boss",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_face_pin_document(path: &str) -> Result<()> {
    let part = bracket_face_pin()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_face_pin_001")?,
        "Bracket with Face Pin",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_bracket_edge_fillet_document(path: &str) -> Result<()> {
    let part = bracket_edge_fillet()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_edge_fillet_001")?,
        "Bracket with Edge Fillet",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_revolve_bushing_document(path: &str) -> Result<()> {
    let part = revolve_bushing()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:revolve_bushing_001")?,
        "Revolved Bushing",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = revolve_parameters("360 deg");
    write_ocad(path, &doc)
}

pub fn create_revolve_sector_document(path: &str) -> Result<()> {
    let part = revolve_sector()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:revolve_sector_001")?,
        "Revolved Sector (180°)",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = revolve_parameters("180 deg");
    write_ocad(path, &doc)
}

pub fn create_bracket_hole_row_document(path: &str) -> Result<()> {
    let part = bracket_hole_row()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_hole_row_001")?,
        "Bracket with Pin Hole Row",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_hole_ring_document(path: &str) -> Result<()> {
    let part = bracket_hole_ring()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_hole_ring_001")?,
        "Bracket with Pin Hole Ring",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_pin_row_document(path: &str) -> Result<()> {
    let part = bracket_pin_row()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_pin_row_001")?,
        "Bracket with Pin Boss Row",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_pin_ring_document(path: &str) -> Result<()> {
    let part = bracket_pin_ring()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_pin_ring_001")?,
        "Bracket with Pin Boss Ring",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    write_ocad(path, &doc)
}

pub fn create_bracket_pin_mirror_document(path: &str) -> Result<()> {
    let part = bracket_pin_mirror()?;
    let metadata = DocumentMetadata::new(
        DocumentId::new("doc:bracket_pin_mirror_001")?,
        "Bracket with Mirrored Pin",
    );
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = bracket_parameters();
    doc.semantic_refs = bracket_semantic_refs();
    write_ocad(path, &doc)
}

pub fn create_assembly_two_brackets_document(path: &str) -> Result<()> {
    use opencad_core::{ComponentId, InstanceId, MateId, TopoRefId};
    use std::path::Path;

    let root = Path::new(path);
    let child_relative = "parts/bracket.ocad.d";
    let child_path = root.join(child_relative);
    create_bracket_document(child_path.to_str().expect("child path"))?;

    let assembly = AssemblyModel {
        components: vec![Component::new(
            ComponentId::new("component:bracket")?,
            child_relative,
            DocumentId::new("doc:bracket_001")?,
        )],
        instances: vec![
            Instance::new(
                InstanceId::new("instance:left")?,
                ComponentId::new("component:bracket")?,
                Placement::identity(),
                "Left Bracket",
            ),
            Instance::new(
                InstanceId::new("instance:right")?,
                ComponentId::new("component:bracket")?,
                Placement::new(RigidTransform::from_translation([0.05, 0.0, 0.0])),
                "Right Bracket",
            ),
        ],
        mates: vec![
            Mate::new(
                MateId::new("mate:ground_left")?,
                MateKind::Ground {
                    instance: InstanceId::new("instance:left")?,
                },
            ),
            Mate::new(
                MateId::new("mate:spacing")?,
                MateKind::Distance {
                    a: MateEntity::at_origin(
                        InstanceId::new("instance:left")?,
                        opencad_geometry::TopoRef::face(
                            TopoRefId::new("ref:face:left_origin")?,
                            "feature:extrude_base",
                            "origin",
                        ),
                    ),
                    b: MateEntity::at_origin(
                        InstanceId::new("instance:right")?,
                        opencad_geometry::TopoRef::face(
                            TopoRefId::new("ref:face:right_origin")?,
                            "feature:extrude_base",
                            "origin",
                        ),
                    ),
                    distance_m: 0.12,
                },
            ),
        ],
        ..Default::default()
    }
    .sorted_deterministic();

    let doc = OcadDocument {
        metadata: DocumentMetadata::new_assembly(
            DocumentId::new("doc:assembly_two_brackets")?,
            "Two Brackets Assembly",
        ),
        parameters: ParamGraph::new(),
        sketches: Vec::new(),
        feature_graph: FeatureGraph::new(),
        feature_nodes: Vec::new(),
        semantic_refs: Vec::new(),
        assertions: Vec::new(),
        assembly: Some(assembly),
        drawing: None,
        attachments: Default::default(),
    };

    write_ocad(path, &doc)
}

fn write_robot_arm_part(
    path: &str,
    part: opencad_feature::PartModel,
    parameters: opencad_graph::ParamGraph,
    doc_id: &str,
    name: &str,
) -> Result<()> {
    let metadata = DocumentMetadata::new(DocumentId::new(doc_id)?, name);
    let mut doc = OcadDocument::from_part_model(metadata, &part);
    doc.parameters = parameters;
    write_ocad(path, &doc)
}

pub fn create_robot_arm_assembly_document(path: &str) -> Result<()> {
    use opencad_assembly::robot_arm;
    use std::path::Path;

    let root = Path::new(path);
    for (child_relative, part, parameters, doc_id, name) in [
        (
            robot_arm::BASE_PATH,
            robot_arm_base()?,
            opencad_graph::robot_arm_base_parameters(),
            robot_arm::BASE_DOC,
            "Robot Arm Base",
        ),
        (
            robot_arm::UPPER_ARM_PATH,
            robot_arm_upper_arm()?,
            opencad_graph::robot_arm_upper_arm_parameters(),
            robot_arm::UPPER_ARM_DOC,
            "Robot Arm Upper Link",
        ),
        (
            robot_arm::FOREARM_PATH,
            robot_arm_forearm()?,
            opencad_graph::robot_arm_forearm_parameters(),
            robot_arm::FOREARM_DOC,
            "Robot Arm Forearm Link",
        ),
        (
            robot_arm::GRIPPER_PATH,
            robot_arm_gripper()?,
            opencad_graph::robot_arm_gripper_parameters(),
            robot_arm::GRIPPER_DOC,
            "Robot Arm Wrist Gripper",
        ),
    ] {
        write_robot_arm_part(
            root.join(child_relative).to_str().expect("child path"),
            part,
            parameters,
            doc_id,
            name,
        )?;
    }

    let assembly = robot_arm_assembly_model()?;
    let doc = OcadDocument {
        metadata: DocumentMetadata::new_assembly(
            DocumentId::new("doc:robot_arm_assembly_001")?,
            "Articulated Robot Arm",
        ),
        parameters: ParamGraph::new(),
        sketches: Vec::new(),
        feature_graph: FeatureGraph::new(),
        feature_nodes: Vec::new(),
        semantic_refs: Vec::new(),
        assertions: Vec::new(),
        assembly: Some(assembly),
        drawing: None,
        attachments: Default::default(),
    };

    write_ocad(path, &doc)
}

pub fn create_bracket_front_view_document(path: &str) -> Result<()> {
    use opencad_core::{DimensionId, SheetId, ViewId};
    use opencad_drawing::{
        DrawingModel, DrawingView, LinearDimension, ModelReference, ProjectionKind, Sheet,
        A4_HEIGHT_M, A4_WIDTH_M,
    };
    use std::path::Path;

    let root = Path::new(path);
    let child_relative = "parts/bracket.ocad.d";
    let child_path = root.join(child_relative);
    create_bracket_document(child_path.to_str().expect("child path"))?;

    let drawing = DrawingModel {
        sheets: vec![Sheet {
            id: SheetId::new("sheet:a4")?,
            name: "Sheet 1".into(),
            width_m: A4_WIDTH_M,
            height_m: A4_HEIGHT_M,
            views: vec![DrawingView::new(
                ViewId::new("view:front")?,
                "Front",
                ModelReference::new(child_relative, DocumentId::new("doc:bracket_001")?),
                ProjectionKind::Front,
                1.0,
                [0.05, 0.05],
            )],
            dimensions: vec![LinearDimension {
                id: DimensionId::new("dim:overall_width")?,
                view_id: ViewId::new("view:front")?,
                start_model_m: [0.0, 0.0, 0.0],
                end_model_m: [0.08, 0.0, 0.0],
                offset_m: -0.01,
            }],
        }],
    }
    .sorted_deterministic();

    let doc = OcadDocument::from_drawing_model(
        DocumentMetadata::new_drawing(
            DocumentId::new("doc:bracket_front_view")?,
            "Bracket Front View",
        ),
        drawing,
    );

    write_ocad(path, &doc)
}
