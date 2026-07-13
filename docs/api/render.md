# Rendering API

## Presentation overlays

`presentation_overlay(scene, overlay)` returns a `SketchOverlay` containing:

- a floor grid sized from the scene bounding-box diagonal;
- deduplicated boundary and B-Rep feature edges;
- the source sketch overlay when supplied.

## Orbit animation

`AnimationOptions` controls `width_px`, `height_px`, `frame_count`,
`frames_per_second`, `orbit_degrees`, `pitch_degrees`, and `show_sketch`.
`AnimationOptions::camera` returns a deterministic camera for a frame index.

`render_orbit_gif` accepts an `OffscreenRenderer`, `RenderScene`, optional
`SketchOverlay`, explicit options, and an output path. It returns
`AnimationSummary` after writing a looping GIF.

The CLI exposes the same capability:

```bash
opencad animate model.ocad.d showcase.gif \
  --width 960 --height 540 \
  --frames 36 --fps 12 \
  --orbit-deg 220 --pitch-deg 26
```

Add `--show-sketch` to include sketch entities and constraint labels.
