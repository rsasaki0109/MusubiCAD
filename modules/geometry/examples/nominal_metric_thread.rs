//! Profile arithmetic only: cargo run -p opencad-geometry --example nominal_metric_thread
use opencad_core::{Angle, Length, Result};
use opencad_geometry::{MetricThreadSize, NominalMetricThread, ThreadHandedness};

fn main() -> Result<()> {
    let thread = NominalMetricThread::new(MetricThreadSize::M6, ThreadHandedness::Right, 8)?;
    println!("Nominal flat-root basic profile only; no kernel solid or tolerance class.");
    println!(
        "M6: major {:.6} mm, pitch {:.6} mm, basic root {:.9} mm, length {:.6} mm",
        thread.major_diameter().millimeters(),
        thread.pitch().millimeters(),
        thread.basic_minor_diameter().millimeters(),
        thread.length().millimeters()
    );
    println!(
        "Phase-zero radius {:.6} mm; ideal envelope volume {:.9} mm^3",
        thread
            .radius_at(Length::from_meters(0.0), Angle::from_radians(0.0))?
            .millimeters(),
        thread.ideal_volume_m3() * 1e9
    );
    Ok(())
}
