# Neutral-plane draft

`DraftFeature` (MCAD-P11-002, ADR-036) tilts selected planar side faces of a
convex solid about their intersections with a neutral plane.

```json
{
  "type": "draft",
  "target_feature": "feature:extrude",
  "angle": { "value_si": 0.08726646259971647 },
  "angle_expr": "draft_angle",
  "neutral_plane": { "type": "datum_plane", "feature_id": "feature:datum" },
  "face_refs": ["ref:face:plate_right"]
}
```

Angle storage is radians; expressions accept `5 deg` or angle parameters.
Positive angles narrow in the neutral normal direction, negative widen.
The neutral intersection remains fixed. Replace the feature via DesignPatch
to change its plane or faces; parameter edits invalidate downstream outputs.
Missing, suppressed, wrong-kind or ambiguous dependencies fail regeneration.

OCCT supports 4–64 planar faces on one convex solid without holes/cavities,
with selected faces initially perpendicular to the neutral plane. Magnitudes
from 1e-8 rad to 45 deg are accepted; collapsed or excessive extents fail.
See ADR-036 for geometry tolerances and the guard-box limit. Mock provides
deterministic contract execution, not physical mass validation.

`examples/draft_plate.ocad.d` and
`examples/agent/create_draft_plate_patch.json` construct a 20×10×4 mm plate
at z=5 mm with 5 degree inward draft on +x. Its volume is
`20*10*4 - 10*4²*tan(5 deg)/2 = 793.0009069179 mm³`.
The same plane may be a global plane, a valid custom plane, a semantic face
plane or a datum plane. Faces are selected through semantic references,
never raw OCCT handles or viewport indices.
