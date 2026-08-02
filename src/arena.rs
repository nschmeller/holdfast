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

#[derive(Clone, Copy, Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn field(items: Vec<PlacedObstacle>) -> ObstacleField {
        ObstacleField { items }
    }

    fn circle(pos: Vec2, r: f32) -> PlacedObstacle {
        PlacedObstacle {
            pos,
            shape: ColliderShape::Circle(r),
            blocks_shots: true,
            height: 1.0,
        }
    }

    fn rect(pos: Vec2, hx: f32, hz: f32, deg: f32) -> PlacedObstacle {
        PlacedObstacle {
            pos,
            shape: ColliderShape::rect_rot(hx, hz, deg),
            blocks_shots: true,
            height: 1.0,
        }
    }

    #[test]
    fn bounding_radius_covers_the_shape() {
        assert_eq!(ColliderShape::Circle(2.0).bounding_radius(), 2.0);
        let r = ColliderShape::rect(3.0, 4.0).bounding_radius();
        assert!((r - 5.0).abs() < 1e-5, "half-extent diagonal, got {r}");
    }

    #[test]
    fn distant_circle_does_not_penetrate() {
        assert!(penetration(Vec2::new(10.0, 0.0), 0.5, &circle(Vec2::ZERO, 1.0)).is_none());
    }

    #[test]
    fn overlapping_circles_push_apart_exactly() {
        // Centres 1.0 apart, radii sum to 1.5, so the push must be 0.5 along +X.
        let push = penetration(Vec2::new(1.0, 0.0), 0.5, &circle(Vec2::ZERO, 1.0)).unwrap();
        assert!((push - Vec2::new(0.5, 0.0)).length() < 1e-5, "{push:?}");
    }

    #[test]
    fn concentric_circles_still_produce_a_push() {
        // The degenerate case: no direction information, but it must not
        // return zero or NaN or the actor is stuck forever.
        let push = penetration(Vec2::ZERO, 0.5, &circle(Vec2::ZERO, 1.0)).unwrap();
        assert!(push.length() > 0.0);
        assert!(push.is_finite());
    }

    #[test]
    fn rect_pushes_out_along_the_nearest_face() {
        let ob = rect(Vec2::ZERO, 1.0, 1.0, 0.0);
        let push = penetration(Vec2::new(1.2, 0.0), 0.5, &ob).unwrap();
        assert!(push.x > 0.0, "should push out in +X, got {push:?}");
        assert!(push.y.abs() < 1e-5);
    }

    #[test]
    fn point_inside_a_rect_escapes_by_the_shallowest_axis() {
        // Inside a wide, thin box: the way out is along Z, not X.
        let ob = rect(Vec2::ZERO, 4.0, 0.5, 0.0);
        let push = penetration(Vec2::new(0.0, 0.1), 0.2, &ob).unwrap();
        assert!(push.y.abs() > push.x.abs(), "{push:?}");
    }

    #[test]
    fn dead_centre_of_a_rect_picks_a_stable_direction() {
        let ob = rect(Vec2::ZERO, 1.0, 2.0, 0.0);
        let push = penetration(Vec2::ZERO, 0.3, &ob).unwrap();
        assert!(push.is_finite());
        assert!(push.length() > 0.0);
        // The X half-extent is smaller, so the escape should be along X.
        assert!(push.x.abs() > push.y.abs());
    }

    #[test]
    fn rotated_rect_penetration_follows_the_rotation() {
        // A 90-degree rotation swaps the effective extents.
        let ob = rect(Vec2::ZERO, 4.0, 0.5, 90.0);
        assert!(
            penetration(Vec2::new(0.0, 3.0), 0.2, &ob).is_some(),
            "a point along +Z should now be inside the rotated box"
        );
        assert!(
            penetration(Vec2::new(3.0, 0.0), 0.2, &ob).is_none(),
            "a point along +X should now be outside"
        );
    }

    #[test]
    fn resolve_leaves_the_actor_clear_of_a_single_obstacle() {
        let f = field(vec![circle(Vec2::ZERO, 2.0)]);
        let out = f.resolve(Vec2::new(0.4, 0.2), 0.5);
        assert!(out.length() >= 2.5 - 1e-3, "ended at {out:?}");
    }

    #[test]
    fn resolve_handles_a_corner_between_two_obstacles() {
        // The classic case the second pass exists for: pushing out of one prop
        // shoves the actor into its neighbour.
        let f = field(vec![
            circle(Vec2::new(-1.0, 0.0), 1.2),
            circle(Vec2::new(1.0, 0.0), 1.2),
        ]);
        let out = f.resolve(Vec2::new(0.0, 0.1), 0.4);
        for ob in &f.items {
            let gap = out.distance(ob.pos);
            assert!(gap > 1.0, "still overlapping {ob:?} at {out:?} (gap {gap})");
        }
    }

    #[test]
    fn resolve_is_a_no_op_in_open_space() {
        let f = field(vec![circle(Vec2::new(50.0, 50.0), 1.0)]);
        let start = Vec2::new(1.0, 2.0);
        assert_eq!(f.resolve(start, 0.5), start);
    }

    #[test]
    fn overlaps_agrees_with_penetration() {
        let f = field(vec![circle(Vec2::ZERO, 1.0)]);
        assert!(f.overlaps(Vec2::new(0.5, 0.0), 0.2));
        assert!(!f.overlaps(Vec2::new(5.0, 0.0), 0.2));
    }

    #[test]
    fn a_segment_through_an_obstacle_is_blocked() {
        let f = field(vec![circle(Vec2::ZERO, 1.0)]);
        assert!(f.blocks_segment(Vec2::new(-5.0, 0.0), Vec2::new(5.0, 0.0), 0.5));
    }

    #[test]
    fn a_segment_beside_an_obstacle_is_clear() {
        let f = field(vec![circle(Vec2::ZERO, 1.0)]);
        assert!(!f.blocks_segment(Vec2::new(-5.0, 4.0), Vec2::new(5.0, 4.0), 0.5));
    }

    #[test]
    fn low_props_do_not_stop_shots() {
        let mut low = circle(Vec2::ZERO, 1.0);
        low.height = 0.2;
        let f = field(vec![low]);
        assert!(
            !f.blocks_segment(Vec2::new(-5.0, 0.0), Vec2::new(5.0, 0.0), 0.5),
            "a shot at height 0.5 should clear a 0.2-high prop"
        );
    }

    #[test]
    fn transparent_props_do_not_stop_shots() {
        let mut clear = circle(Vec2::ZERO, 1.0);
        clear.blocks_shots = false;
        let f = field(vec![clear]);
        assert!(!f.blocks_segment(Vec2::new(-5.0, 0.0), Vec2::new(5.0, 0.0), 0.5));
    }

    #[test]
    fn a_degenerate_segment_tests_its_own_point() {
        let f = field(vec![circle(Vec2::ZERO, 1.0)]);
        assert!(f.blocks_segment(Vec2::ZERO, Vec2::ZERO, 0.5));
        assert!(!f.blocks_segment(Vec2::new(9.0, 9.0), Vec2::new(9.0, 9.0), 0.5));
    }

    #[test]
    fn clear_empties_the_field() {
        let mut f = field(vec![circle(Vec2::ZERO, 1.0)]);
        f.clear();
        assert!(f.items.is_empty());
        assert!(!f.overlaps(Vec2::ZERO, 5.0));
    }

    #[test]
    fn bounds_clamp_keeps_the_radius_inside() {
        let b = ArenaBounds {
            half_x: 10.0,
            half_z: 5.0,
        };
        let p = b.clamp(Vec2::new(100.0, -100.0), 1.0);
        assert_eq!(p, Vec2::new(9.0, -4.0));
    }

    #[test]
    fn bounds_contains_uses_the_true_edge() {
        let b = ArenaBounds {
            half_x: 10.0,
            half_z: 5.0,
        };
        assert!(b.contains(Vec2::new(10.0, 5.0)));
        assert!(!b.contains(Vec2::new(10.1, 0.0)));
    }

    #[test]
    fn edge_distance_is_positive_inside_and_negative_outside() {
        let b = ArenaBounds {
            half_x: 10.0,
            half_z: 5.0,
        };
        assert!((b.edge_distance(Vec2::ZERO) - 5.0).abs() < 1e-6);
        assert!(b.edge_distance(Vec2::new(12.0, 0.0)) < 0.0);
    }

    #[test]
    fn perimeter_points_land_on_the_boundary() {
        let b = ArenaBounds {
            half_x: 10.0,
            half_z: 5.0,
        };
        for i in 0..400 {
            let p = b.perimeter_point(i as f32 / 400.0);
            let on_edge = (p.x.abs() - b.half_x).abs() < 1e-3
                || (p.y.abs() - b.half_z).abs() < 1e-3;
            assert!(on_edge, "t={i} gave {p:?}");
            assert!(b.contains(p + Vec2::splat(1e-4)) || on_edge);
        }
    }

    #[test]
    fn perimeter_wraps_around() {
        let b = ArenaBounds::default();
        let a = b.perimeter_point(0.25);
        let c = b.perimeter_point(1.25);
        assert!((a - c).length() < 1e-3);
        // Negative parameters must wrap too, not clamp.
        assert!((b.perimeter_point(-0.75) - a).length() < 1e-3);
    }

    #[test]
    fn perimeter_covers_all_four_edges() {
        let b = ArenaBounds {
            half_x: 10.0,
            half_z: 5.0,
        };
        let pts: Vec<Vec2> = (0..200).map(|i| b.perimeter_point(i as f32 / 200.0)).collect();
        assert!(pts.iter().any(|p| p.y <= -b.half_z + 1e-3));
        assert!(pts.iter().any(|p| p.y >= b.half_z - 1e-3));
        assert!(pts.iter().any(|p| p.x <= -b.half_x + 1e-3));
        assert!(pts.iter().any(|p| p.x >= b.half_x - 1e-3));
    }

    #[test]
    fn diagonal_matches_the_rectangle() {
        let b = ArenaBounds {
            half_x: 3.0,
            half_z: 4.0,
        };
        assert!((b.diagonal() - 10.0).abs() < 1e-5);
    }

    #[test]
    fn gust_only_affects_its_lane_while_blowing() {
        let mut g = Gust {
            blowing: false,
            lane_center_z: 4.0,
            lane_half_width: 2.0,
            ..Gust::default()
        };
        assert!(!g.affects(Vec2::new(0.0, 4.0)), "a still gust affects nothing");
        g.blowing = true;
        assert!(g.affects(Vec2::new(0.0, 4.0)));
        assert!(g.affects(Vec2::new(99.0, 6.0)), "lanes are unbounded in X");
        assert!(!g.affects(Vec2::new(0.0, 7.0)));
    }

    #[test]
    fn spotlight_respects_its_radius_and_switch() {
        let mut s = Spotlight {
            center: Vec2::new(2.0, 2.0),
            radius: 3.0,
            enabled: true,
            ..Spotlight::default()
        };
        assert!(s.contains(Vec2::new(2.0, 4.0)));
        assert!(!s.contains(Vec2::new(2.0, 9.0)));
        s.enabled = false;
        assert!(!s.contains(Vec2::new(2.0, 2.0)), "disabled means never inside");
    }

    #[test]
    fn hazard_constructors_set_sensible_defaults() {
        let scald = Hazard::scald(2.0, 10.0);
        assert!(scald.hurts_player && scald.hurts_enemies);
        assert_eq!(scald.slow, 1.0);
        assert!(scald.life.is_none());

        let sticky = Hazard::sticky(1.0, 0.5).with_life(3.0).enemies_only();
        assert!(!sticky.hurts_player);
        assert_eq!(sticky.life, Some(3.0));

        let mine = Hazard::scald(1.0, 5.0).player_only();
        assert!(!mine.hurts_enemies);
    }
}
