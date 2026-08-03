//! Who is fighting whom.
//!
//! The world is not one horde. It is divided into **regions**, each held by a
//! monster faction, assigned deterministically from the world seed - so the
//! same rule serves a fixed arena and an unbounded world equally, and walking
//! somewhere new means walking into somebody else's ground.
//!
//! Monster factions are **neutral to one another** by default and hostile only
//! to the player. That neutrality is a resource: late research lets the player
//! [`Diplomacy::incite`] two of them into a war for a while, and the right
//! moment to spend it is the whole point of reading the map.

use bevy::prelude::*;

use crate::{AppState, GameSet, RunSetup};

/// Side of one region lattice cell, in world units.
///
/// Four chunks across. Big enough that a region is a place you travel through
/// rather than a tile you step over, small enough that a long run crosses
/// several.
pub const REGION_CELL: f32 = 96.0;

/// Everyone who can own ground, hold a fort, or be shot at.
///
/// The player is a faction like any other. That is what lets a captured fort
/// work for its new owner without a second code path.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Faction {
    Player,
    Swarm,
    Rust,
    Bloom,
    Void,
}

impl Faction {
    pub const COUNT: usize = 5;
    pub const ALL: [Self; Self::COUNT] = [
        Self::Player,
        Self::Swarm,
        Self::Rust,
        Self::Bloom,
        Self::Void,
    ];
    /// Everyone except the player. Regions are drawn from these.
    pub const MONSTERS: [Self; 4] = [Self::Swarm, Self::Rust, Self::Bloom, Self::Void];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Player => "YOURS",
            Self::Swarm => "THE SWARM",
            Self::Rust => "THE RUST",
            Self::Bloom => "THE BLOOM",
            Self::Void => "THE VOID",
        }
    }

    /// Short label for map markers and the HUD.
    pub fn tag(self) -> &'static str {
        match self {
            Self::Player => "YOU",
            Self::Swarm => "SWARM",
            Self::Rust => "RUST",
            Self::Bloom => "BLOOM",
            Self::Void => "VOID",
        }
    }

    /// Banner colour. Every fort, nest and banner-bearer wears it, so a glance
    /// across the field says whose ground you are standing on.
    pub fn color(self) -> Color {
        match self {
            Self::Player => Color::srgb(0.36, 0.86, 0.52),
            Self::Swarm => Color::srgb(0.98, 0.66, 0.22),
            Self::Rust => Color::srgb(0.86, 0.32, 0.24),
            Self::Bloom => Color::srgb(0.85, 0.35, 0.78),
            Self::Void => Color::srgb(0.45, 0.55, 1.0),
        }
    }

    /// How this faction fights. The same fort logic reads differently through
    /// each of these, which is what stops four factions being one faction in
    /// four colours.
    pub fn temperament(self) -> Temperament {
        match self {
            // The player's holdings are steady: they do not self-expand, since
            // expansion is the player's decision to make.
            Self::Player => Temperament {
                expansion: 0.6,
                garrison: 1.0,
                ambition: 0.5,
                blurb: "Whatever you have taken and can keep.",
            },
            Self::Swarm => Temperament {
                expansion: 1.6,
                garrison: 0.45,
                ambition: 0.75,
                blurb: "Spreads fast, holds badly. Numbers instead of walls.",
            },
            Self::Rust => Temperament {
                expansion: 0.55,
                garrison: 1.5,
                ambition: 0.4,
                blurb: "Digs in. Slow to expand, miserable to evict.",
            },
            Self::Bloom => Temperament {
                expansion: 1.9,
                garrison: 0.7,
                ambition: 0.35,
                blurb: "Seeds relentlessly. Ignore it and the map is theirs.",
            },
            Self::Void => Temperament {
                expansion: 0.8,
                garrison: 0.6,
                ambition: 1.5,
                blurb: "Few, strong, and always massing on something.",
            },
        }
    }
}

/// The dials that make each faction play differently.
#[derive(Debug, Clone, Copy)]
pub struct Temperament {
    /// How readily it sends out seeders to plant new nests.
    pub expansion: f32,
    /// How much strength it keeps at home rather than sending out.
    pub garrison: f32,
    /// How readily it masses on a fort instead of hunting the player.
    pub ambition: f32,
    pub blurb: &'static str,
}

/// Which faction owns a thing. Absent means nobody does.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allegiance(pub Faction);

/// Which faction holds the ground at a world position.
///
/// A jittered Voronoi over a coarse lattice: each cell contributes one site,
/// and the nearest site wins. Cheap, seamless, unbounded, and it produces
/// borders that wander instead of running along a grid.
#[must_use]
pub fn faction_at(pos: Vec2, seed: u64) -> Faction {
    let home = IVec2::new(
        (pos.x / REGION_CELL).floor() as i32,
        (pos.y / REGION_CELL).floor() as i32,
    );

    let mut best = (f32::INFINITY, Faction::Swarm);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let cell = home + IVec2::new(dx, dz);
            let (site, faction) = region_site(cell, seed);
            let d = site.distance_squared(pos);
            if d < best.0 {
                best = (d, faction);
            }
        }
    }
    best.1
}

/// The site one lattice cell contributes, and whose it is.
fn region_site(cell: IVec2, seed: u64) -> (Vec2, Faction) {
    let hash = mix(seed, cell);
    // Keep the jitter inside the middle of the cell. A site that can reach the
    // border lets one region swallow its neighbour entirely.
    let jx = ((hash & 0xFFFF) as f32 / 65535.0).mul_add(0.6, 0.2);
    let jz = (((hash >> 16) & 0xFFFF) as f32 / 65535.0).mul_add(0.6, 0.2);
    let site = Vec2::new(
        (cell.x as f32 + jx) * REGION_CELL,
        (cell.y as f32 + jz) * REGION_CELL,
    );
    let pick = ((hash >> 32) as usize) % Faction::MONSTERS.len();
    (site, Faction::MONSTERS[pick])
}

/// Avalanche a coordinate into a well-spread hash.
///
/// The same sequential-mix shape as `world::chunk_rng`, and for the same
/// reason: xor-combining the axes is commutative, so coordinates collide in
/// pairs and the map comes out visibly regular.
fn mix(seed: u64, cell: IVec2) -> u64 {
    let mut hash = seed ^ 0x51D5_C0DE_D00D;
    for value in [i64::from(cell.x) as u64, i64::from(cell.y) as u64] {
        hash ^= value.wrapping_add(0x9E37_79B9_7F4A_7C15);
        hash = hash.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
        hash ^= hash >> 29;
    }
    hash
}

/// Who is at war with whom.
///
/// Monster factions ignore each other unless the player has set them against
/// one another. Keeping that in one resource means every targeting decision
/// asks the same question and none of them can disagree.
#[derive(Resource, Debug, Default)]
pub struct Diplomacy {
    /// Seconds of forced hostility remaining for each unordered pair.
    wars: [[f32; Faction::COUNT]; Faction::COUNT],
    /// Wars that started or ended this frame, for the HUD to announce.
    pub announce: Vec<String>,
}

impl Diplomacy {
    pub fn reset(&mut self) {
        self.wars = [[0.0; Faction::COUNT]; Faction::COUNT];
        self.announce.clear();
    }

    /// Whether `a` will attack `b` on sight.
    #[must_use]
    pub fn hostile(&self, a: Faction, b: Faction) -> bool {
        if a == b {
            return false;
        }
        // Everything is always hostile to the player, and the player to
        // everything. That never needs inciting.
        if a == Faction::Player || b == Faction::Player {
            return true;
        }
        self.wars[a.index()][b.index()] > 0.0
    }

    /// Set two monster factions against each other for `seconds`.
    ///
    /// Inciting the player against anything is meaningless - that war is
    /// permanent - so it is quietly ignored rather than treated as an error.
    pub fn incite(&mut self, a: Faction, b: Faction, seconds: f32) {
        if a == b || a == Faction::Player || b == Faction::Player {
            return;
        }
        let fresh = self.wars[a.index()][b.index()] <= 0.0;
        let until = self.wars[a.index()][b.index()].max(seconds);
        self.wars[a.index()][b.index()] = until;
        self.wars[b.index()][a.index()] = until;
        if fresh {
            self.announce
                .push(format!("{} TURNS ON {}", a.name(), b.name()));
        }
    }

    /// How long `a` and `b` stay at war, in seconds. Zero when they are not.
    #[must_use]
    pub fn war_remaining(&self, a: Faction, b: Faction) -> f32 {
        if a == b {
            return 0.0;
        }
        self.wars[a.index()][b.index()]
    }

    /// Every war currently running, as `(a, b, seconds left)`.
    pub fn active_wars(&self) -> Vec<(Faction, Faction, f32)> {
        let mut out = Vec::new();
        for (i, a) in Faction::ALL.iter().enumerate() {
            for b in Faction::ALL.iter().skip(i + 1) {
                let left = self.wars[a.index()][b.index()];
                if left > 0.0 {
                    out.push((*a, *b, left));
                }
            }
        }
        out
    }

    fn tick(&mut self, dt: f32) {
        for (i, a) in Faction::ALL.iter().enumerate() {
            for b in Faction::ALL.iter().skip(i + 1) {
                let left = &mut self.wars[a.index()][b.index()];
                if *left <= 0.0 {
                    continue;
                }
                *left -= dt;
                if *left <= 0.0 {
                    *left = 0.0;
                    self.wars[b.index()][a.index()] = 0.0;
                    self.announce
                        .push(format!("{} AND {} STAND DOWN", a.name(), b.name()));
                }
            }
        }
        // Mirror the lower triangle so `hostile` can read either order.
        for (i, a) in Faction::ALL.iter().enumerate() {
            for b in Faction::ALL.iter().skip(i + 1) {
                self.wars[b.index()][a.index()] = self.wars[a.index()][b.index()];
            }
        }
    }
}

/// Ask for a war between whichever two factions it would most help to have
/// fighting. Raised by research; resolved here, where the map is known.
#[derive(Message, Debug, Clone, Copy)]
pub struct InciteRequest {
    pub seconds: f32,
}

#[derive(Debug)]
pub struct FactionPlugin;

impl Plugin for FactionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Diplomacy>()
            .add_message::<InciteRequest>()
            .add_systems(Update, resolve_incitements.in_set(GameSet::Think))
            .add_systems(
                OnExit(AppState::Menu),
                reset_diplomacy.in_set(RunSetup::Reset),
            )
            .add_systems(Update, tick_diplomacy.in_set(GameSet::Input));
    }
}

fn reset_diplomacy(mut diplomacy: ResMut<Diplomacy>) {
    diplomacy.reset();
}

fn tick_diplomacy(
    time: Res<Time>,
    mut diplomacy: ResMut<Diplomacy>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
) {
    diplomacy.tick(time.delta_secs());
    for line in std::mem::take(&mut diplomacy.announce) {
        hints.push(
            line,
            "They are not looking at you.",
            crate::onboarding::HintTone::Discovery,
        );
    }
}

/// The two factions worth setting against each other, given how much of each
/// is nearby.
///
/// Both have to actually be present. Inciting a war between two powers the
/// player cannot see is a line of text, not a tactic, and they paid Cores for
/// it.
#[must_use]
pub fn pick_feuding_pair(weight: &[f32; Faction::COUNT]) -> Option<(Faction, Faction)> {
    let mut ranked: Vec<(Faction, f32)> = Faction::MONSTERS
        .iter()
        .map(|f| (*f, weight[f.index()]))
        .collect();
    ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
    match (ranked.first(), ranked.get(1)) {
        (Some(&(a, wa)), Some(&(b, wb))) if wa > 0.0 && wb > 0.0 => Some((a, b)),
        _ => None,
    }
}

/// Turn "start a war" into "start *that* war".
///
/// Picks the two factions with the most strength near the player, because a
/// war between two powers on the far side of the map is a line of text rather
/// than a tactic. The player spent Cores on this; it has to land somewhere
/// they will see it.
fn resolve_incitements(
    mut requests: MessageReader<InciteRequest>,
    mut diplomacy: ResMut<Diplomacy>,
    mut hints: ResMut<crate::onboarding::HintQueue>,
    mut records: MessageWriter<crate::stats::Record>,
    player: Query<&crate::common::Body, With<crate::player::Player>>,
    monsters: Query<(&crate::common::Body, &Allegiance)>,
) {
    for request in requests.read() {
        let Some(hero) = player.iter().next().map(|b| b.pos) else {
            continue;
        };

        let mut weight = [0.0f32; Faction::COUNT];
        for (body, allegiance) in &monsters {
            // Nearer bodies count for more, so "strongest nearby" means what
            // it says rather than "biggest faction anywhere".
            let d = body.pos.distance(hero);
            weight[allegiance.0.index()] += 1.0 / (1.0 + d / 40.0);
        }

        if let Some((a, b)) = pick_feuding_pair(&weight) {
            diplomacy.incite(a, b, request.seconds);
            records.write(crate::stats::Record::add(
                crate::stats::stat::WARS_STARTED,
                1.0,
            ));
        } else {
            hints.push(
                "NOBODY TO TURN",
                "There is only one power in earshot. Travel, then try again.",
                crate::onboarding::HintTone::Tip,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: u64 = 0x00FA_C710;

    #[test]
    fn regions_are_stable_for_a_seed() {
        // Walking away and back must not find a different owner.
        for p in [
            Vec2::ZERO,
            Vec2::new(310.0, -180.0),
            Vec2::new(-999.0, 42.0),
        ] {
            assert_eq!(faction_at(p, SEED), faction_at(p, SEED));
        }
    }

    #[test]
    fn a_different_seed_redraws_the_map() {
        let a: Vec<_> = (0..40)
            .map(|i| faction_at(Vec2::new(i as f32 * 37.0, 0.0), SEED))
            .collect();
        let b: Vec<_> = (0..40)
            .map(|i| faction_at(Vec2::new(i as f32 * 37.0, 0.0), SEED ^ 0x99))
            .collect();
        assert_ne!(a, b, "the region map ignores its seed");
    }

    #[test]
    fn every_monster_faction_gets_ground() {
        // A faction that never owns anything may as well not exist.
        let mut seen = std::collections::HashSet::new();
        for x in -12..12 {
            for z in -12..12 {
                seen.insert(faction_at(
                    Vec2::new(x as f32 * REGION_CELL, z as f32 * REGION_CELL),
                    SEED,
                ));
            }
        }
        for faction in Faction::MONSTERS {
            assert!(seen.contains(&faction), "{faction:?} holds nothing");
        }
        assert!(
            !seen.contains(&Faction::Player),
            "the player does not start owning ground"
        );
    }

    #[test]
    fn regions_are_contiguous_enough_to_read_as_places() {
        // Sample a line and count how often the owner changes. A map that
        // flickers between factions every few steps is noise, not territory.
        let mut changes = 0;
        let mut last = faction_at(Vec2::ZERO, SEED);
        let steps = 600;
        for i in 1..steps {
            let here = faction_at(Vec2::new(i as f32 * 4.0, 0.0), SEED);
            if here != last {
                changes += 1;
                last = here;
            }
        }
        // 2400 units of travel; regions are ~96 across, so a couple of dozen
        // crossings is expected and a couple of hundred would be noise.
        assert!(changes > 3, "only {changes} borders in 2400 units");
        assert!(changes < 60, "{changes} borders in 2400 units is static");
    }

    #[test]
    fn monster_factions_ignore_each_other_until_incited() {
        let mut d = Diplomacy::default();
        assert!(!d.hostile(Faction::Swarm, Faction::Rust));
        d.incite(Faction::Swarm, Faction::Rust, 30.0);
        assert!(d.hostile(Faction::Swarm, Faction::Rust));
        assert!(d.hostile(Faction::Rust, Faction::Swarm), "war is symmetric");
        // Bystanders are unaffected.
        assert!(!d.hostile(Faction::Bloom, Faction::Void));
    }

    #[test]
    fn everything_is_always_hostile_to_the_player() {
        let d = Diplomacy::default();
        for faction in Faction::MONSTERS {
            assert!(d.hostile(faction, Faction::Player));
            assert!(d.hostile(Faction::Player, faction));
        }
    }

    #[test]
    fn nothing_is_hostile_to_itself() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Swarm, Faction::Swarm, 30.0);
        for faction in Faction::ALL {
            assert!(!d.hostile(faction, faction));
        }
    }

    #[test]
    fn a_war_expires_and_they_stand_down() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Bloom, Faction::Void, 10.0);
        d.announce.clear();
        for _ in 0..50 {
            d.tick(0.25);
        }
        assert!(!d.hostile(Faction::Bloom, Faction::Void));
        assert!(
            d.announce.iter().any(|l| l.contains("STAND DOWN")),
            "the end of a war should be announced too"
        );
    }

    #[test]
    fn inciting_again_extends_rather_than_shortens() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Swarm, Faction::Rust, 30.0);
        d.tick(20.0);
        d.incite(Faction::Swarm, Faction::Rust, 30.0);
        assert!(d.war_remaining(Faction::Swarm, Faction::Rust) >= 29.9);
    }

    #[test]
    fn a_shorter_incitement_does_not_cut_a_longer_war_short() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Swarm, Faction::Rust, 60.0);
        d.incite(Faction::Swarm, Faction::Rust, 5.0);
        assert!(d.war_remaining(Faction::Swarm, Faction::Rust) > 50.0);
    }

    #[test]
    fn inciting_against_the_player_is_a_no_op() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Player, Faction::Swarm, 30.0);
        assert!(d.active_wars().is_empty(), "that war needs no declaring");
    }

    #[test]
    fn active_wars_lists_each_pair_once() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Swarm, Faction::Rust, 30.0);
        d.incite(Faction::Bloom, Faction::Void, 20.0);
        assert_eq!(d.active_wars().len(), 2);
    }

    #[test]
    fn resetting_ends_every_war() {
        let mut d = Diplomacy::default();
        d.incite(Faction::Swarm, Faction::Rust, 30.0);
        d.reset();
        assert!(d.active_wars().is_empty());
        assert!(!d.hostile(Faction::Swarm, Faction::Rust));
    }

    #[test]
    fn every_faction_is_described_and_distinct() {
        let mut tags: Vec<_> = Faction::ALL.iter().map(|f| f.tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), Faction::COUNT);

        for faction in Faction::ALL {
            assert!(!faction.name().is_empty());
            assert!(faction.tag().len() <= 5, "{faction:?} tag will not fit");
            assert!(!faction.temperament().blurb.is_empty());
        }

        // Colours have to be tellable apart at a glance across a battlefield.
        for a in Faction::ALL {
            for b in Faction::ALL {
                if a.index() >= b.index() {
                    continue;
                }
                let (x, y) = (a.color().to_linear(), b.color().to_linear());
                let delta =
                    (x.red - y.red).abs() + (x.green - y.green).abs() + (x.blue - y.blue).abs();
                assert!(delta > 0.25, "{a:?} and {b:?} look the same");
            }
        }
    }

    #[test]
    fn temperaments_actually_differ() {
        // Four factions with the same dials are one faction in four colours.
        for a in Faction::MONSTERS {
            for b in Faction::MONSTERS {
                if a.index() >= b.index() {
                    continue;
                }
                let (x, y) = (a.temperament(), b.temperament());
                let delta = (x.expansion - y.expansion).abs()
                    + (x.garrison - y.garrison).abs()
                    + (x.ambition - y.ambition).abs();
                assert!(delta > 0.3, "{a:?} and {b:?} play the same");
            }
        }
    }

    #[test]
    fn the_faction_index_matches_the_declaration_order() {
        // `Diplomacy` indexes its matrix by this, so a mismatch would silently
        // declare war between the wrong parties.
        for (i, faction) in Faction::ALL.iter().enumerate() {
            assert_eq!(faction.index(), i, "{faction:?} is out of order");
        }
    }
    fn weights(pairs: &[(Faction, f32)]) -> [f32; Faction::COUNT] {
        let mut w = [0.0; Faction::COUNT];
        for (f, v) in pairs {
            w[f.index()] = *v;
        }
        w
    }

    #[test]
    fn a_feud_needs_two_powers_actually_present() {
        // One faction in earshot is not a war, it is a mugging.
        assert_eq!(pick_feuding_pair(&weights(&[])), None);
        assert_eq!(pick_feuding_pair(&weights(&[(Faction::Swarm, 9.0)])), None);
    }

    #[test]
    fn a_feud_picks_the_two_strongest_nearby() {
        let w = weights(&[
            (Faction::Swarm, 1.0),
            (Faction::Rust, 8.0),
            (Faction::Bloom, 5.0),
            (Faction::Void, 0.2),
        ]);
        assert_eq!(pick_feuding_pair(&w), Some((Faction::Rust, Faction::Bloom)));
    }

    #[test]
    fn a_feud_ignores_factions_that_are_not_there() {
        let w = weights(&[(Faction::Void, 3.0), (Faction::Swarm, 0.5)]);
        assert_eq!(pick_feuding_pair(&w), Some((Faction::Void, Faction::Swarm)));
    }
}
