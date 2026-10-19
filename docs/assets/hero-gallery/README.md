# Reproduce the opening GIF gallery

These ten animations come from the checked-in example documents and the native
MusubiCAD CLI. The generator does not synthesize or retouch CAD geometry.

From the repository root, build the current CLI and run:

```bash
cargo build -p opencad-cli
python -m pip install Pillow==12.3.0
python docs/assets/generate-hero-gallery.py --musubicad target/debug/musubicad
```

Use `target/debug/musubicad.exe` on Windows. The eight CPU animations need no GPU.
The circular-pattern orbit and feature-build animation use the existing offscreen
renderer; on headless Linux, install `libvulkan1` and `mesa-vulkan-drivers` for
Mesa's software Vulkan adapter. The generator inherits the CLI's normal runtime
environment and runs commands from the repository root.

| Clip | Example | What changes |
|---|---|---|
| [Six-axis arm](six-axis-arm.gif) | [six_axis_arm](../../../examples/six_axis_arm.ocad.d) | Six source joints between two explicit poses, returning to zero |
| [Actuator hub](actuator-hub.gif) | [robot_joint_actuator](../../../examples/robot_joint_actuator.ocad.d) | Upper hub height, 24–60 mm |
| [Bearing bore](bearing-bore.gif) | [bearing_carrier](../../../examples/bearing_carrier.ocad.d) | Bore diameter, 8–26 mm |
| [Revolve angle](revolve-angle.gif) | [physical_rectangular_revolve](../../../examples/physical_rectangular_revolve.ocad.d) | Turn angle about the source datum axis, 60–330° |
| [Cylindrical draft](cylindrical-draft.gif) | [cylindrical_draft](../../../examples/cylindrical_draft.ocad.d) | Draft angle about the source neutral plane, 2–14° |
| [Tapered rib](tapered-rib.gif) | [tapered_rib_plate](../../../examples/tapered_rib_plate.ocad.d) | Rib height, 8–26 mm |
| [Ellipse profile](ellipse-profile.gif) | [ellipse_plate](../../../examples/ellipse_plate.ocad.d) | Minor radius, 8–24 mm; major radius stays 30 mm |
| [Variable fillet](variable-fillet.gif) | [variable_fillet_plate](../../../examples/variable_fillet_plate.ocad.d) | Start radius, 0.5–2.5 mm; end radius stays 1.5 mm |
| [Circular pattern](circular-pattern.gif) | [bracket_pin_ring](../../../examples/bracket_pin_ring.ocad.d) | A 360° camera orbit about the regenerated part |
| [Feature build](feature-build.gif) | [robot_joint_actuator](../../../examples/robot_joint_actuator.ocad.d) | Regenerated body milestones in Feature Graph order |

Parameter sweeps use the existing validated, in-memory `set_parameter`
DesignPatch path and regenerate every sampled value with OpenCASCADE. Joint
animation uses the source kinematic tree and enforces its declared limits; it is
not a collision-clearance certification. Presentation orbits move only the camera.
The existing feature-build renderer reveals actual regenerated body milestones.
No example document is written by these commands.

Every GIF is 600 × 400 pixels, loops indefinitely, and contains distinct frames
with positive durations. Clips are capped at 1.5 MB each and 6 MB combined. The
generator decodes each result, checks these limits and verifies all source-file
hashes before publishing it. [manifest.json](manifest.json) records the exact CLI
commands, input hashes, output hashes, decoded frame counts and download size.
Pillow is used only to inspect the GIFs, not to alter their pixels.

To preview one clip without changing the full-gallery manifest:

```bash
python docs/assets/generate-hero-gallery.py --musubicad target/debug/musubicad --only revolve-angle
```

The original [Claude Code session hero](../agent-demo/hero.gif) remains separate,
with its own [transcript](../agent-demo/hero-session.json) and
[generator](../generate-hero-demo.py).
