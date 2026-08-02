//! Arena mechanics shared by every environment: bounds, solid props, hazards.
//!
//! Collision is deliberately 2D. Everything walks on the ground plane, so we
//! resolve circles against circles and oriented rectangles in the XZ plane and
//! let the third dimension be purely presentational. That keeps hundreds of
//! enemies cheap and sidesteps a whole class of 3D pathing bugs.
//!
//! Nothing here knows what a desk is - the concrete environments live in
//! `environments/` and only supply data.

use bevy::prelude::*;

/// Playfield extents, set by whichever environment is loaded.
#[derive(Resource, Clone, Copy)]
pub struct ArenaBounds {
    pub half_x: f32,
    pub half_z: f32,
}

impl Default for ArenaBounds {
    fn default() -> Self {
        Self {
            half_x: 20.0,
            half_z: 13.0,
        }
    }
}

impl ArenaBounds {
    /// Keep a circle of `radius` inside the playfield.
    pub fn clamp(&self, pos: Vec2, radius: f32) -> Vec2 {
        Vec2::new(
            pos.x.clamp(-self.half_x + radius, self.half_x - radius),
            pos.y.clamp(-self.half_z + radius, self.half_z - radius),
        )
    }

    pub fn contains(&self, pos: Vec2) -> bool {
        pos.x.abs() <= self.half_x && pos.y.abs() <= self.half_z
    }

    /// Distance to the nearest edge; negative once past it.
    pub fn edge_distance(&self, pos: Vec2) -> f32 {
        (self.half_x - pos.x.abs()).min(self.half_z - pos.y.abs())
    }

    /// A point on the perimeter at parameter `t` in `[0, 1)`, used for spawning
    /// waves evenly around the rim.
    pub fn perimeter_point(&self, t: f32) -> Vec2 {
        let t = t.rem_euclid(1.0);
        let (w, h) = (self.half_x * 2.0, self.half_z * 2.0);
        let perim = (w + h) * 2.0;
        let d = t * perim;
        if d < w {
            Vec2::new(-self.half_x + d, -self.half_z)
        } else if d < w + h {
            Vec2::new(self.half_x, -self.half_z + (d - w))
        } else if d < w * 2.0 + h {
            Vec2::new(self.half_x - (d - w - h), self.half_z)
        } else {
            Vec2::new(-self.half_x, self.half_z - (d - w * 2.0 - h))
        }
    }

    pub fn diagonal(&self) -> f32 {
        Vec2::new(self.half_x, self.half_z).length() * 2.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ColliderShape {
    Circle(f32),
    /// Axis-aligned in local space, rotated by `rot` radians about Y.
    Rect { half: Vec2, rot: f32 },
}

impl ColliderShape {
    pub fn rect(hx: f32, hz: f32) -> Self {
        Self::Rect {
            half: Vec2::new(hx, hz),
            rot: 0.0,
        }
    }

    pub fn rect_rot(hx: f32, hz: f32, deg: f32) -> Self {
        Self::Rect {
            half: Vec2::new(hx, hz),
            rot: deg.to_radians(),
        }
    }

    pub fn bounding_radius(&self) -> f32 {
        match self {
            Self::Circle(r) => *r,
            Self::Rect { half, .. } => half.length(),
        }
    }
}

/// Cached obstacle list, rebuilt whenever an environment loads. Iterating a
/// flat `Vec` beats querying the world once per actor per frame.
#[derive(Resource, Default)]
pub struct ObstacleField {
    pub items: Vec<PlacedObstacle>,
}

#[derive(Clone, Copy)]
pub struct PlacedObstacle {
    pub pos: Vec2,
    pub shape: ColliderShape,
    /// Tall props stop projectiles; flat ones (rugs, decals) do not.
    pub blocks_shots: bool,
    /// Visual height, used to decide whether a shot passes over it.
    pub height: f32,
}

impl ObstacleField {
    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn push(&mut self, pos: Vec2, shape: ColliderShape, blocks_shots: bool, height: f32) {
        self.items.push(PlacedObstacle {
            pos,
            shape,
            blocks_shots,
            height,
        });
    }

    /// Push a circle of `radius` at `pos` out of every overlapping obstacle.
    pub fn resolve(&self, mut pos: Vec2, radius: f32) -> Vec2 {
        // Two passes: one correction can push an actor into a neighbouring
        // prop, and in a field this dense that happens constantly at corners.
        for _ in 0..2 {
            let mut moved = false;
            for ob in &self.items {
                if let Some(push) = penetration(pos, radius, ob) {
                    pos += push;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        pos
    }

    pub fn overlaps(&self, pos: Vec2, radius: f32) -> bool {
        self.items
            .iter()
            .any(|ob| penetration(pos, radius, ob).is_some())
    }

    /// True if the segment `a -> b` is interrupted by a shot-blocking prop.
    pub fn blocks_segment(&self, a: Vec2, b: Vec2, shot_height: f32) -> bool {
        self.items
            .iter()
            .any(|ob| ob.blocks_shots && ob.height >= shot_height && segment_hits(a, b, ob))
    }
}

/// Depenetration vector for a circle against one obstacle, or `None` if clear.
fn penetration(pos: Vec2, radius: f32, ob: &PlacedObstacle) -> Option<Vec2> {
    match ob.shape {
        ColliderShape::Circle(r) => {
            let delta = pos - ob.pos;
            let dist_sq = delta.length_squared();
            let min = r + radius;
            if dist_sq >= min * min {
                return None;
            }
            let dist = dist_sq.sqrt();
            // Exactly concentric: pick an arbitrary but stable direction.
            let dir = if dist > 1e-5 { delta / dist } else { Vec2::X };
            Some(dir * (min - dist))
        }
        ColliderShape::Rect { half, rot } => {
            let (sin, cos) = rot.sin_cos();
            let delta = pos - ob.pos;
            // Into the rect's local frame.
            let local = Vec2::new(delta.x * cos + delta.y * sin, -delta.x * sin + delta.y * cos);
            let clamped = local.clamp(-half, half);
            let offset = local - clamped;
            let dist_sq = offset.length_squared();

            let local_push = if dist_sq > 1e-8 {
                if dist_sq >= radius * radius {
                    return None;
                }
                let dist = dist_sq.sqrt();
                (offset / dist) * (radius - dist)
            } else {
                // Centre is inside the box: escape along the shallowest axis.
                let dx = half.x - local.x.abs();
                let dz = half.y - local.y.abs();
                if dx < dz {
                    Vec2::new((dx + radius) * sign_or_pos(local.x), 0.0)
                } else {
                    Vec2::new(0.0, (dz + radius) * sign_or_pos(local.y))
                }
            };

            // Back to world space.
            Some(Vec2::new(
                local_push.x * cos - local_push.y * sin,
                local_push.x * sin + local_push.y * cos,
            ))
        }
    }
}

/// `f32::signum` returns +1 for +0.0 but -1 for -0.0; dead centre should pick a
/// stable direction rather than depend on the sign of zero.
#[inline]
fn sign_or_pos(v: f32) -> f32 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

/// Cheap segment/obstacle test. We sample along the segment rather than doing
/// exact swept intersection: shots are small and fast, and a sample every half
/// unit is well under the size of anything we place.
fn segment_hits(a: Vec2, b: Vec2, ob: &PlacedObstacle) -> bool {
    let seg = b - a;
    let len = seg.length();
    if len < 1e-4 {
        return penetration(a, 0.05, ob).is_some();
    }
    let steps = ((len / 0.5).ceil() as usize).clamp(1, 64);
    (0..=steps).any(|i| {
        let p = a + seg * (i as f32 / steps as f32);
        penetration(p, 0.05, ob).is_some()
    })
}

// -- hazards ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HazardKind {
    /// Burns whatever stands in it.
    Scald,
    /// Slows whatever stands in it.
    Sticky,
    /// Brief stun pulse.
    Shock,
    /// Heals whatever stands in it - which includes the enemy. Carried by a
    /// negative `dps`, so the damage pipeline needs no special case beyond a
    /// sign check.
    Font,
}

#[derive(Component)]
pub struct Hazard {
    pub kind: HazardKind,
    pub radius: f32,
    pub dps: f32,
    /// Multiplier applied to movement speed inside the zone.
    pub slow: f32,
    /// `None` for permanent environment features.
    pub life: Option<f32>,
    pub hurts_player: bool,
    pub hurts_enemies: bool,
}

impl Hazard {
    pub fn scald(radius: f32, dps: f32) -> Self {
        Self {
            kind: HazardKind::Scald,
            radius,
            dps,
            slow: 1.0,
            life: None,
            hurts_player: true,
            hurts_enemies: true,
        }
    }

    pub fn sticky(radius: f32, slow: f32) -> Self {
        Self {
            kind: HazardKind::Sticky,
            radius,
            dps: 0.0,
            slow,
            life: None,
            hurts_player: true,
            hurts_enemies: true,
        }
    }

    pub fn with_life(mut self, life: f32) -> Self {
        self.life = Some(life);
        self
    }

    pub fn enemies_only(mut self) -> Self {
        self.hurts_player = false;
        self
    }

    pub fn player_only(mut self) -> Self {
        self.hurts_enemies = false;
        self
    }
}

/// A recurring directional sweep across one lane of the arena: the desk's USB
/// fan, the forest's wind, the rooftop's downdraft, the grid's gravity shear.
#[derive(Resource)]
pub struct Gust {
    pub interval: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub remaining: f32,
    pub blowing: bool,
    pub dir: Vec2,
    pub lane_center_z: f32,
    pub lane_half_width: f32,
    pub strength: f32,
    pub enabled: bool,
    pub label: &'static str,
}

impl Default for Gust {
    fn default() -> Self {
        Self {
            interval: 12.0,
            duration: 2.6,
            cooldown: 12.0,
            remaining: 0.0,
            blowing: false,
            dir: Vec2::new(-1.0, 0.0),
            lane_center_z: 6.0,
            lane_half_width: 4.0,
            strength: 13.0,
            enabled: true,
            label: "GUST",
        }
    }
}

impl Gust {
    pub fn affects(&self, pos: Vec2) -> bool {
        self.blowing && (pos.y - self.lane_center_z).abs() <= self.lane_half_width
    }
}

/// A pool of light that rewards standing in it. Standing there is a real
/// choice: more damage, but it is also where the director aims its elites.
#[derive(Resource)]
pub struct Spotlight {
    pub center: Vec2,
    pub radius: f32,
    pub damage_bonus: f32,
    pub enabled: bool,
    pub label: &'static str,
}

impl Default for Spotlight {
    fn default() -> Self {
        Self {
            center: Vec2::new(12.5, -8.5),
            radius: 6.0,
            damage_bonus: 0.25,
            enabled: true,
            label: "LAMPLIGHT",
        }
    }
}

impl Spotlight {
    pub fn contains(&self, pos: Vec2) -> bool {
        self.enabled && pos.distance_squared(self.center) <= self.radius * self.radius
    }
}
