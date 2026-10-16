//! ADR-100: guarded physical runtime, not a document source operation.
#[path = "support/faceted_circular_fillet.rs"]
mod operation;
fn main() -> opencad_core::Result<()> {
    println!("{}", operation::report()?);
    Ok(())
}
