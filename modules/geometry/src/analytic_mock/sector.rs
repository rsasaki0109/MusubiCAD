//! ADR-103: exact bounded one-arc/two-radii sectors, not polygon mass.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct SectorProfile {
    pub(super) frame: Frame,
    pub(super) radius: f64,
    pub(super) sweep: f64,
}
#[derive(Clone, Debug)]
pub(super) struct Sector {
    pub(super) profile: SectorProfile,
    pub(super) height: f64,
}
impl SectorProfile {
    pub(super) fn from_sketch(sketch: &SolvedSketch) -> Result<Self> {
        let p = crate::arc_sector::profile(sketch)?;
        Ok(Self {
            frame: Frame {
                origin: p.origin_m,
                axes: p.axes,
            },
            radius: p.radius_m,
            sweep: p.sweep_rad,
        })
    }
    pub(super) fn extruded(&self, height: f64, direction: [f64; 3], shift: [f64; 3]) -> Sector {
        Sector {
            profile: Self {
                frame: Frame {
                    origin: add(self.frame.origin, shift),
                    axes: [self.frame.axes[0], self.frame.axes[1], direction],
                },
                radius: self.radius,
                sweep: self.sweep,
            },
            height,
        }
    }
}
impl Sector {
    pub(super) fn at(&self, angle: f64, z: f64) -> [f64; 3] {
        self.profile.frame.map([
            self.profile.radius * angle.cos(),
            self.profile.radius * angle.sin(),
            z,
        ])
    }
    pub(super) fn mass(&self) -> Result<(f64, f64, [f64; 3])> {
        mesh::validate_frame(self.profile.frame)?;
        let (r, t, h) = (self.profile.radius, self.profile.sweep, self.height);
        let area = r * r * t / 2.;
        let distance = 4. * r * (t / 2.).sin() / (3. * t);
        Ok((
            area * h,
            2. * area + r * (t + 2.) * h,
            self.profile
                .frame
                .map([distance * (t / 2.).cos(), distance * (t / 2.).sin(), h / 2.]),
        ))
    }
    pub(super) fn bounds(&self) -> BoundingBox {
        let frame = self.profile.frame;
        let mut angles = vec![0., self.profile.sweep];
        for i in 0..3 {
            let a = frame.axes[1][i].atan2(frame.axes[0][i]).rem_euclid(2. * PI);
            for angle in [a, (a + PI).rem_euclid(2. * PI)] {
                if angle <= self.profile.sweep {
                    angles.push(angle);
                }
            }
        }
        let mut bounds = BoundingBox {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        };
        for z in [0., self.height] {
            for p in
                std::iter::once(frame.map([0., 0., z])).chain(angles.iter().map(|a| self.at(*a, z)))
            {
                for (i, v) in p.into_iter().enumerate() {
                    bounds.min[i] = bounds.min[i].min(v);
                    bounds.max[i] = bounds.max[i].max(v);
                }
            }
        }
        bounds
    }
    pub(super) fn transformed(&self, transform: RigidTransform) -> Self {
        Self {
            profile: SectorProfile {
                frame: self.profile.frame.transformed(transform),
                ..self.profile.clone()
            },
            height: self.height,
        }
    }
}
