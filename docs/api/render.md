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

## Shaded previews

`OffscreenRenderer` images (`opencad screenshot`, `opencad animate`, and the
headless preview) add two view-independent cues to the lit solid:

- **Feature edges.** Mesh boundary edges and creases sharper than 25° are
  drawn in near-black over the solid, depth-tested against it. Holes, bosses,
  and plate thickness read at any camera angle.
- **Baked ambient occlusion.** Each vertex colour is darkened by a
  24-direction hemisphere occlusion term. The rays search 16 % of the scene
  diagonal, and a fully occluded vertex keeps 15 % of its colour. The
  directions come from a Fibonacci spiral, so bakes are deterministic.
  The bake is brute force: scenes that need more than 10^8 ray/triangle tests
  render without it. The last bake is reused while the mesh is unchanged, so
  orbit animations bake once.

The interactive viewport and GPU picking are unchanged.
