//! Levels, upgrade cards, research, and gear.
//!
//! The run is endless, so nothing here has a ceiling. The card pool eventually
//! runs dry of new content and starts offering Refinements; the research tree
//! ends in repeatable nodes with rising costs; weapons cap at eight and then
//! evolve. The curve never flattens, it just changes what it is made of.

use bevy::prelude::*;

use crate::allies::{AllyKind, Economy};
use crate::environments::EnvKind;
use crate::palette as pal;
use crate::player::PlayerStats;
use crate::rng::Rng;
use crate::weapons::{Loadout, MAX_LEVEL, WeaponKind};
use crate::{AppState, GameSet, RunSetup};

// -- experience -------------------------------------------------------------

#[derive(Debug, Resource)]
pub struct Progression {
    pub level: u32,
    pub xp: f32,
    pub to_next: f32,
    /// Levels banked but not yet spent on a card.
    pub pending_levels: u32,
    pub skill_points: u32,
    pub total_xp: f64,
}

impl Default for Progression {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0.0,
            to_next: 12.0,
            pending_levels: 0,
            skill_points: 0,
            total_xp: 0.0,
        }
    }
}

impl Progression {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn gain(&mut self, amount: f32) {
        self.xp += amount;
        self.total_xp += f64::from(amount);
        // Bounded rather than `while`: a single enormous XP grant should not
        // be able to spin here, and nothing legitimately crosses 64 levels at
        // once.
        for _ in 0..64 {
            if self.xp < self.to_next {
                break;
            }
            self.xp -= self.to_next;
            self.level += 1;
            self.pending_levels += 1;
            // Every third level also funds the research tree.
            if self.level.is_multiple_of(3) {
                self.skill_points += 1;
            }
            // Superlinear, and steeper than it was: the old curve let a good
            // build bank a level every twenty seconds indefinitely, so the
            // card count ran away from everything it was supposed to be
            // measured against.
            self.to_next = 12.0 * (1.0 + self.level as f32 * 0.34).powf(1.32);
        }
    }

    pub fn fraction(&self) -> f32 {
        (self.xp / self.to_next).clamp(0.0, 1.0)
    }
}

// -- upgrade cards ----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Card {
    pub title: String,
    pub detail: String,
    pub kind: CardKind,
    pub rarity: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum CardKind {
    NewWeapon(WeaponKind),
    LevelWeapon(WeaponKind),
    Stat(StatBoost),
    /// Post-pool filler: small stacking bonus to everything.
    Refinement,
    FreeCores(f32),
    FreeScrap(f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatBoost {
    MaxHp,
    MoveSpeed,
    Damage,
    Haste,
    Area,
    Crit,
    Armor,
    Regen,
    Pickup,
    XpGain,
    Luck,
    Knockback,
    AllyPower,
    StructurePower,
    BuildDiscount,
    Income,
    CaptureRate,
    ProjectileCount,
}

impl StatBoost {
    pub const ALL: [Self; 18] = [
        Self::MaxHp,
        Self::MoveSpeed,
        Self::Damage,
        Self::Haste,
        Self::Area,
        Self::Crit,
        Self::Armor,
        Self::Regen,
        Self::Pickup,
        Self::XpGain,
        Self::Luck,
        Self::Knockback,
        Self::AllyPower,
        Self::StructurePower,
        Self::BuildDiscount,
        Self::Income,
        Self::CaptureRate,
        Self::ProjectileCount,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::MaxHp => "Thicker Shell",
            Self::MoveSpeed => "Waxed Feet",
            Self::Damage => "Sharpened",
            Self::Haste => "Caffeinated",
            Self::Area => "Wider Reach",
            Self::Crit => "Weak Points",
            Self::Armor => "Plating",
            Self::Regen => "Second Wind",
            Self::Pickup => "Magnetised",
            Self::XpGain => "Fast Learner",
            Self::Luck => "Lucky Streak",
            Self::Knockback => "Heavy Hands",
            Self::AllyPower => "Drill Sergeant",
            Self::StructurePower => "Overtuned",
            Self::BuildDiscount => "Bulk Salvage",
            Self::Income => "Logistics",
            Self::CaptureRate => "Flag Bearer",
            Self::ProjectileCount => "Split Shot",
        }
    }

    fn detail(self) -> &'static str {
        match self {
            Self::MaxHp => "+22 max health, and heal that much now.",
            Self::MoveSpeed => "+8% movement speed.",
            Self::Damage => "+14% weapon damage.",
            Self::Haste => "+12% fire rate.",
            Self::Area => "+12% weapon area and range.",
            Self::Crit => "+6% critical chance.",
            Self::Armor => "+2 flat damage reduction.",
            Self::Regen => "+0.7 health per second.",
            Self::Pickup => "+40% pickup radius.",
            Self::XpGain => "+15% experience.",
            Self::Luck => "+10% chance to roll a better drop.",
            Self::Knockback => "+25% knockback. Edges become weapons.",
            Self::AllyPower => "+20% ally damage and health.",
            Self::StructurePower => "+20% structure damage and health.",
            Self::BuildDiscount => "Structures cost 12% less Scrap.",
            Self::Income => "+20% Scrap and Core income.",
            Self::CaptureRate => "+30% zone capture speed.",
            Self::ProjectileCount => "+1 projectile on weapons that fan.",
        }
    }

    fn apply(self, stats: &mut PlayerStats) {
        match self {
            Self::MaxHp => stats.max_hp += 22.0,
            Self::MoveSpeed => stats.move_speed *= 1.08,
            Self::Damage => stats.damage_mult *= 1.14,
            Self::Haste => stats.haste *= 1.12,
            Self::Area => stats.area *= 1.12,
            Self::Crit => stats.crit_chance = (stats.crit_chance + 0.06).min(0.95),
            Self::Armor => stats.armor += 2.0,
            Self::Regen => stats.regen += 0.7,
            Self::Pickup => stats.pickup_radius *= 1.4,
            Self::XpGain => stats.xp_mult *= 1.15,
            Self::Luck => stats.luck += 0.1,
            Self::Knockback => stats.knockback *= 1.25,
            Self::AllyPower => {
                stats.ally_damage *= 1.2;
                stats.ally_health *= 1.2;
            }
            Self::StructurePower => {
                stats.structure_damage *= 1.2;
                stats.structure_health *= 1.2;
            }
            Self::BuildDiscount => {
                stats.build_discount = (stats.build_discount + 0.12).min(0.7);
            }
            Self::Income => {
                stats.income_mult *= 1.2;
                stats.scrap_mult *= 1.2;
                stats.core_mult *= 1.2;
            }
            Self::CaptureRate => stats.zone_capture_rate *= 1.3,
            Self::ProjectileCount => stats.extra_projectiles += 1,
        }
    }
}

/// The three cards currently on offer.
#[derive(Debug, Resource, Default)]
pub struct CardOffer {
    pub cards: Vec<Card>,
    /// One free reroll per level-up, because a dead offer is not a decision.
    pub reroll_available: bool,
}

// -- research tree ----------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Branch {
    Might,
    Swift,
    Vital,
    Command,
}

impl Branch {
    pub const ALL: [Self; 4] = [Self::Might, Self::Swift, Self::Vital, Self::Command];

    pub fn title(self) -> &'static str {
        match self {
            Self::Might => "MIGHT",
            Self::Swift => "SWIFT",
            Self::Vital => "VITAL",
            Self::Command => "COMMAND",
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Might => Color::srgb(1.0, 0.45, 0.35),
            Self::Swift => Color::srgb(0.45, 0.9, 1.0),
            Self::Vital => Color::srgb(0.5, 1.0, 0.6),
            Self::Command => Color::srgb(1.0, 0.78, 0.35),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResearchNode {
    pub branch: Branch,
    pub title: &'static str,
    pub detail: &'static str,
    pub rank: u32,
    pub max_rank: u32,
    /// Base Core cost; rises with each rank taken.
    pub cost: f32,
    pub boost: StatBoost,
    /// Repeatable end-of-branch nodes never cap out.
    pub endless: bool,
    /// Non-zero on nodes that set two monster factions at war when bought.
    ///
    /// Research is otherwise a wall of percentages. This is the one line in
    /// the tree that does something to the *world* rather than to the player,
    /// and it is the payoff for having read the map: turn the neighbours on
    /// each other and walk through the middle while they are busy.
    pub discord: f32,
}

impl ResearchNode {
    pub fn current_cost(&self) -> f32 {
        self.cost * (1.0 + self.rank as f32 * if self.endless { 0.65 } else { 0.4 })
    }

    pub fn maxed(&self) -> bool {
        !self.endless && self.rank >= self.max_rank
    }
}

#[derive(Debug, Resource)]
pub struct Research {
    pub nodes: Vec<ResearchNode>,
    pub cursor: usize,
}

impl Default for Research {
    fn default() -> Self {
        Self {
            nodes: default_nodes(),
            cursor: 0,
        }
    }
}

impl Research {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn in_branch(&self, branch: Branch) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.branch == branch)
            .map(|(i, _)| i)
            .collect()
    }
}

fn default_nodes() -> Vec<ResearchNode> {
    use Branch::{Command, Might, Swift, Vital};
    let n = |branch, title, detail, max_rank, cost, boost, endless| ResearchNode {
        branch,
        title,
        detail,
        rank: 0,
        max_rank,
        cost,
        boost,
        endless,
        discord: 0.0,
    };
    let discord = |title, detail, cost, boost, seconds| ResearchNode {
        branch: Command,
        title,
        detail,
        rank: 0,
        max_rank: 1,
        cost,
        boost,
        endless: true,
        discord: seconds,
    };
    vec![
        // MIGHT
        n(
            Might,
            "Honed Edge",
            "+10% weapon damage",
            5,
            3.0,
            StatBoost::Damage,
            false,
        ),
        n(
            Might,
            "Weak Points",
            "+5% critical chance",
            4,
            4.0,
            StatBoost::Crit,
            false,
        ),
        n(
            Might,
            "Broad Swing",
            "+10% area",
            4,
            4.0,
            StatBoost::Area,
            false,
        ),
        n(
            Might,
            "Endless Edge",
            "+8% damage, forever",
            0,
            8.0,
            StatBoost::Damage,
            true,
        ),
        // SWIFT
        n(
            Swift,
            "Quick Hands",
            "+10% fire rate",
            5,
            3.0,
            StatBoost::Haste,
            false,
        ),
        n(
            Swift,
            "Light Step",
            "+6% move speed",
            4,
            3.0,
            StatBoost::MoveSpeed,
            false,
        ),
        n(
            Swift,
            "Extra Barrel",
            "+1 projectile",
            2,
            9.0,
            StatBoost::ProjectileCount,
            false,
        ),
        n(
            Swift,
            "Endless Tempo",
            "+7% fire rate, forever",
            0,
            8.0,
            StatBoost::Haste,
            true,
        ),
        // VITAL
        n(
            Vital,
            "Hard Shell",
            "+20 max health",
            5,
            3.0,
            StatBoost::MaxHp,
            false,
        ),
        n(
            Vital,
            "Plating",
            "+2 armour",
            4,
            4.0,
            StatBoost::Armor,
            false,
        ),
        n(
            Vital,
            "Recovery",
            "+0.6 regen",
            4,
            4.0,
            StatBoost::Regen,
            false,
        ),
        n(
            Vital,
            "Endless Vigour",
            "+18 health, forever",
            0,
            7.0,
            StatBoost::MaxHp,
            true,
        ),
        // COMMAND
        n(
            Command,
            "Drill",
            "+15% ally power",
            5,
            3.0,
            StatBoost::AllyPower,
            false,
        ),
        n(
            Command,
            "Machining",
            "+15% structure power",
            5,
            3.0,
            StatBoost::StructurePower,
            false,
        ),
        n(
            Command,
            "Supply Lines",
            "+15% income",
            4,
            4.0,
            StatBoost::Income,
            false,
        ),
        n(
            Command,
            "Flag Bearer",
            "+25% capture speed",
            3,
            5.0,
            StatBoost::CaptureRate,
            false,
        ),
        n(
            Command,
            "Endless Logistics",
            "+12% income, forever",
            0,
            8.0,
            StatBoost::Income,
            true,
        ),
        // The two nodes that act on the world rather than the player. Late and
        // expensive on purpose: knowing which two factions to set against each
        // other means having explored enough to know who is out there.
        discord(
            "Whisper Campaign",
            "Sets the two strongest nearby factions at war for 45s",
            14.0,
            StatBoost::Income,
            45.0,
        ),
        discord(
            "Blood Feud",
            "As above, but 110s - and they remember it",
            26.0,
            StatBoost::Luck,
            110.0,
        ),
    ]
}

// -- gear -------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GearSlot {
    Head,
    Body,
    Trinket,
}

impl GearSlot {
    pub const ALL: [Self; 3] = [Self::Head, Self::Body, Self::Trinket];

    pub fn label(self) -> &'static str {
        match self {
            Self::Head => "HEAD",
            Self::Body => "BODY",
            Self::Trinket => "TRINKET",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GearPiece {
    pub name: String,
    pub slot: GearSlot,
    pub rarity: usize,
    pub boosts: Vec<(StatBoost, u32)>,
}

impl GearPiece {
    /// A crude power score, used to tell the player whether a drop is an
    /// upgrade without making them read two stat blocks.
    pub fn score(&self) -> u32 {
        self.boosts.iter().map(|(_, n)| n).sum::<u32>() * (self.rarity as u32 + 1)
    }

    pub fn describe(&self) -> String {
        self.boosts
            .iter()
            .map(|(b, n)| format!("{} x{n}", b.title()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Resource, Default)]
pub struct Equipped {
    pub head: Option<GearPiece>,
    pub body: Option<GearPiece>,
    pub trinket: Option<GearPiece>,
}

impl Equipped {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn get(&self, slot: GearSlot) -> Option<&GearPiece> {
        match slot {
            GearSlot::Head => self.head.as_ref(),
            GearSlot::Body => self.body.as_ref(),
            GearSlot::Trinket => self.trinket.as_ref(),
        }
    }

    pub fn set(&mut self, piece: GearPiece) {
        match piece.slot {
            GearSlot::Head => self.head = Some(piece),
            GearSlot::Body => self.body = Some(piece),
            GearSlot::Trinket => self.trinket = Some(piece),
        }
    }
}

const GEAR_NAMES: [[&str; 4]; 3] = [
    [
        "Paper Crown",
        "Bottle Cap Helm",
        "Thimble Helm",
        "Crown of Ink",
    ],
    [
        "Tape Wrap",
        "Foil Vest",
        "Cardboard Plate",
        "Mantle of Reams",
    ],
    [
        "Lucky Clip",
        "Warm Battery",
        "Compass Charm",
        "The Last Staple",
    ],
];

/// Roll a gear piece. `luck` and threat both push the rarity roll upward, which
/// is the main way the pacing dial pays out in power rather than numbers.
pub fn roll_gear(rng: &mut Rng, luck: f32, rarity_bonus: f32) -> GearPiece {
    const MAX_TIER_CHANCE: f32 = 0.85;
    let slot = GearSlot::ALL[rng.below(3)];
    let mut rarity = 0;
    // The cap matters: stacked luck and a maxed threat dial can push the raw
    // sum past 1.0, which would make every single drop Legendary and flatten
    // the whole gear economy. Capping each tier roll keeps the ladder intact
    // while still making luck feel worth taking.
    let mut chance = (0.34 + luck + rarity_bonus).min(MAX_TIER_CHANCE);
    while rarity < 3 && rng.chance(chance) {
        rarity += 1;
        // Each successive upgrade is harder.
        chance = (chance * 0.42).min(MAX_TIER_CHANCE);
    }

    let count = 1 + rarity.min(2);
    let mut boosts = Vec::new();
    for _ in 0..count {
        let boost = StatBoost::ALL[rng.below(StatBoost::ALL.len())];
        let stacks = 1 + rng.below(1 + rarity) as u32;
        boosts.push((boost, stacks));
    }

    GearPiece {
        name: GEAR_NAMES[slot as usize][rarity].to_string(),
        slot,
        rarity,
        boosts,
    }
}

// -- plugin -----------------------------------------------------------------

/// Recompute `PlayerStats` from base + every source. Running this from scratch
/// each time something changes means no source can silently drift.
#[derive(Debug, Message)]
pub struct RecomputeStats;

/// Applied bonuses that persist for the run, accumulated from cards.
#[derive(Debug, Resource, Default)]
pub struct AppliedBoosts {
    pub entries: Vec<(StatBoost, u32)>,
    pub refinements: u32,
}

impl AppliedBoosts {
    pub fn reset(&mut self) {
        self.entries.clear();
        self.refinements = 0;
    }

    pub fn add(&mut self, boost: StatBoost, stacks: u32) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.0 == boost) {
            e.1 += stacks;
        } else {
            self.entries.push((boost, stacks));
        }
    }
}

#[derive(Debug)]
pub struct ProgressPlugin;

impl Plugin for ProgressPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Progression>()
            .init_resource::<CardOffer>()
            .init_resource::<Research>()
            .init_resource::<Equipped>()
            .init_resource::<AppliedBoosts>()
            .add_message::<RecomputeStats>()
            .add_systems(Update, check_level_up.in_set(GameSet::Resolve))
            .add_systems(Update, recompute_stats)
            .add_systems(
                OnExit(AppState::Menu),
                reset_progress.in_set(RunSetup::Reset),
            );
    }
}

fn reset_progress(
    mut progression: ResMut<Progression>,
    mut research: ResMut<Research>,
    mut equipped: ResMut<Equipped>,
    mut boosts: ResMut<AppliedBoosts>,
    mut offer: ResMut<CardOffer>,
    mut recompute: MessageWriter<RecomputeStats>,
) {
    progression.reset();
    research.reset();
    equipped.reset();
    boosts.reset();
    offer.cards.clear();
    recompute.write(RecomputeStats);
}

/// Opens the level-up screen when levels are banked.
fn check_level_up(
    env: Res<EnvKind>,
    progression: Res<Progression>,
    offer: Res<CardOffer>,
    loadout: Res<Loadout>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    boosts: Res<AppliedBoosts>,
) {
    if progression.pending_levels == 0 || !offer.cards.is_empty() {
        return;
    }
    let cards = build_offer(&mut rng, &loadout, &boosts, *env);
    commands.insert_resource(CardOffer {
        cards,
        reroll_available: true,
    });
    next.set(AppState::LevelUp);
}

/// Three distinct options, weighted so a new weapon is exciting but not
/// guaranteed, and never offering the same thing twice.
pub fn build_offer(
    rng: &mut Rng,
    loadout: &Loadout,
    boosts: &AppliedBoosts,
    env: EnvKind,
) -> Vec<Card> {
    let mut pool: Vec<Card> = Vec::new();

    for kind in loadout.offerable() {
        match loadout.level_of(kind) {
            None => pool.push(Card {
                title: kind.name(env).to_string(),
                detail: kind.blurb().to_string(),
                kind: CardKind::NewWeapon(kind),
                rarity: 2,
            }),
            Some(level) => {
                let next = level + 1;
                let detail = if next >= MAX_LEVEL {
                    kind.mastery().to_string()
                } else {
                    format!("Level {next}. More damage, faster, wider.")
                };
                pool.push(Card {
                    title: format!("{} +", kind.name(env)),
                    detail,
                    kind: CardKind::LevelWeapon(kind),
                    rarity: if next >= MAX_LEVEL { 3 } else { 1 },
                });
            }
        }
    }

    for boost in StatBoost::ALL {
        pool.push(Card {
            title: boost.title().to_string(),
            detail: boost.detail().to_string(),
            kind: CardKind::Stat(boost),
            rarity: 0,
        });
    }

    pool.push(Card {
        title: "Salvage Cache".to_string(),
        detail: "Immediately gain 60 Scrap.".to_string(),
        kind: CardKind::FreeScrap(60.0),
        rarity: 0,
    });
    pool.push(Card {
        title: "Core Fragment".to_string(),
        detail: "Immediately gain 3 Cores.".to_string(),
        kind: CardKind::FreeCores(3.0),
        rarity: 1,
    });

    // Once the pool is thin, Refinements keep the offer meaningful forever.
    if pool.len() < 6 {
        pool.push(Card {
            title: format!("Refinement {}", boosts.refinements + 1),
            detail: "+4% to damage, health, fire rate and income.".to_string(),
            kind: CardKind::Refinement,
            rarity: 2,
        });
    }

    rng.shuffle(&mut pool);
    pool.truncate(3);
    pool
}

pub fn apply_card(
    card: &Card,
    stats: &mut PlayerStats,
    loadout: &mut Loadout,
    economy: &mut Economy,
    boosts: &mut AppliedBoosts,
) {
    match card.kind {
        CardKind::NewWeapon(kind) | CardKind::LevelWeapon(kind) => loadout.add(kind),
        CardKind::Stat(boost) => boosts.add(boost, 1),
        CardKind::Refinement => boosts.refinements += 1,
        CardKind::FreeCores(n) => economy.gain_cores(n),
        CardKind::FreeScrap(n) => economy.gain_scrap(n),
    }
    // Health cards should feel immediate.
    if matches!(card.kind, CardKind::Stat(StatBoost::MaxHp)) {
        stats.max_hp += 0.0; // recomputed below; kept explicit for clarity
    }
}

#[allow(clippy::too_many_arguments)]
fn recompute_stats(
    mut events: MessageReader<RecomputeStats>,
    mut stats: ResMut<PlayerStats>,
    boosts: Res<AppliedBoosts>,
    equipped: Res<Equipped>,
    research: Res<Research>,
    mut healths: Query<&mut crate::common::Health, With<crate::player::Player>>,
) {
    if events.is_empty() {
        return;
    }
    events.clear();

    let mut next = PlayerStats::default();

    for (boost, stacks) in &boosts.entries {
        for _ in 0..*stacks {
            boost.apply(&mut next);
        }
    }

    for slot in GearSlot::ALL {
        if let Some(piece) = equipped.get(slot) {
            for (boost, stacks) in &piece.boosts {
                for _ in 0..*stacks {
                    boost.apply(&mut next);
                }
            }
        }
    }

    for node in &research.nodes {
        for _ in 0..node.rank {
            node.boost.apply(&mut next);
        }
    }

    // Refinements touch everything a little.
    let r = 1.0 + f32::from(u16::try_from(boosts.refinements).unwrap_or(u16::MAX)) * 0.04;
    next.damage_mult *= r;
    next.haste *= r;
    next.max_hp *= r;
    next.income_mult *= r;

    let old_max = stats.max_hp;
    *stats = next;

    // Growing the pool should grant the difference, not silently widen the bar.
    if stats.max_hp > old_max {
        for mut health in &mut healths {
            let delta = stats.max_hp - old_max;
            health.max = stats.max_hp;
            health.heal(delta);
        }
    } else {
        for mut health in &mut healths {
            health.max = stats.max_hp;
            health.current = health.current.min(health.max);
        }
    }
}

/// Card colour by rarity, shared with the HUD.
pub fn card_color(rarity: usize) -> Color {
    pal::RARITY[rarity.min(3)]
}

/// Recruit costs shown on the squad panel.
pub fn recruit_hint(economy: &Economy, env: EnvKind) -> String {
    AllyKind::ALL
        .iter()
        .map(|k| {
            let affordable = economy.cores >= k.core_cost();
            format!(
                "{}{} {}",
                if affordable { "" } else { "-" },
                k.name(env),
                k.core_cost() as u32
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> Rng {
        Rng::seeded(0xC0FFEE)
    }

    #[test]
    fn levels_are_earned_and_banked() {
        let mut p = Progression::default();
        assert_eq!(p.level, 1);
        p.gain(12.0);
        assert_eq!(p.level, 2);
        assert_eq!(p.pending_levels, 1);
    }

    #[test]
    fn one_huge_xp_grant_can_cross_several_levels() {
        let mut p = Progression::default();
        p.gain(10_000.0);
        assert!(p.level > 5, "only reached level {}", p.level);
        assert_eq!(
            p.pending_levels,
            p.level - 1,
            "every level gained must bank a card"
        );
    }

    #[test]
    fn the_xp_curve_is_strictly_increasing() {
        let mut p = Progression::default();
        let mut previous = p.to_next;
        for _ in 0..60 {
            p.gain(p.to_next);
            assert!(p.to_next > previous, "curve flattened at level {}", p.level);
            previous = p.to_next;
        }
    }

    #[test]
    fn skill_points_arrive_every_third_level() {
        let mut p = Progression::default();
        while p.level < 10 {
            p.gain(p.to_next);
        }
        // Levels 3, 6 and 9 each grant one.
        assert_eq!(p.skill_points, 3);
    }

    #[test]
    fn fraction_is_always_a_sane_bar_value() {
        let mut p = Progression::default();
        for _ in 0..500 {
            p.gain(7.3);
            let f = p.fraction();
            assert!((0.0..=1.0).contains(&f), "fraction was {f}");
        }
    }

    #[test]
    fn total_xp_accumulates_independently_of_levels() {
        let mut p = Progression::default();
        p.gain(50.0);
        p.gain(50.0);
        assert!((p.total_xp - 100.0).abs() < 1e-6);
    }

    #[test]
    fn reset_clears_everything() {
        let mut p = Progression::default();
        p.gain(5000.0);
        p.reset();
        assert_eq!(p.level, 1);
        assert_eq!(p.pending_levels, 0);
        assert_eq!(p.total_xp, 0.0);
    }

    // -- stat boosts --------------------------------------------------------

    #[test]
    fn every_boost_moves_at_least_one_stat() {
        for boost in StatBoost::ALL {
            let before = PlayerStats::default();
            let mut after = PlayerStats::default();
            boost.apply(&mut after);
            let changed = format!("{before:?}") != format!("{after:?}");
            assert!(changed, "{boost:?} changed nothing");
        }
    }

    #[test]
    fn every_boost_has_a_title_and_a_detail() {
        for boost in StatBoost::ALL {
            assert!(!boost.title().is_empty());
            assert!(!boost.detail().is_empty());
        }
    }

    #[test]
    fn crit_chance_cannot_exceed_its_cap() {
        let mut s = PlayerStats::default();
        for _ in 0..100 {
            StatBoost::Crit.apply(&mut s);
        }
        assert!(s.crit_chance <= 0.95);
    }

    #[test]
    fn build_discount_cannot_make_things_free() {
        let mut s = PlayerStats::default();
        for _ in 0..100 {
            StatBoost::BuildDiscount.apply(&mut s);
        }
        assert!(s.build_discount <= 0.7);
    }

    #[test]
    fn armour_never_makes_the_player_immortal() {
        let s = PlayerStats {
            armor: 10_000.0,
            ..PlayerStats::default()
        };
        // The floor keeps a fraction of every hit no matter the armour.
        assert!(s.mitigate(100.0) >= 12.0 - 1e-3);
    }

    #[test]
    fn armour_reduces_ordinary_hits() {
        let s = PlayerStats {
            armor: 5.0,
            ..PlayerStats::default()
        };
        assert!((s.mitigate(50.0) - 45.0).abs() < 1e-5);
    }

    // -- research -----------------------------------------------------------

    #[test]
    fn the_tree_has_every_branch_populated() {
        let r = Research::default();
        for branch in Branch::ALL {
            assert!(
                !r.in_branch(branch).is_empty(),
                "{} has no nodes",
                branch.title()
            );
        }
    }

    #[test]
    fn every_branch_ends_in_a_repeatable_node() {
        let r = Research::default();
        for branch in Branch::ALL {
            let has_endless = r.in_branch(branch).into_iter().any(|i| r.nodes[i].endless);
            assert!(has_endless, "{} cannot be pushed forever", branch.title());
        }
    }

    #[test]
    fn node_costs_rise_with_rank() {
        let mut node = Research::default().nodes.remove(0);
        let first = node.current_cost();
        node.rank = 3;
        assert!(node.current_cost() > first);
    }

    #[test]
    fn finite_nodes_max_out_and_endless_ones_do_not() {
        let r = Research::default();
        let mut finite = r.nodes.iter().find(|n| !n.endless).unwrap().clone();
        finite.rank = finite.max_rank;
        assert!(finite.maxed());

        let mut endless = r.nodes.iter().find(|n| n.endless).unwrap().clone();
        endless.rank = 9999;
        assert!(!endless.maxed(), "endless nodes must never cap");
    }

    #[test]
    fn branch_titles_and_colours_are_distinct() {
        let titles: Vec<_> = Branch::ALL.iter().map(|b| b.title()).collect();
        let mut sorted = titles.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), titles.len());
    }

    // -- gear ---------------------------------------------------------------

    #[test]
    fn rolled_gear_is_always_well_formed() {
        let mut rng = rng();
        for _ in 0..5000 {
            let g = roll_gear(&mut rng, 0.0, 0.0);
            assert!(g.rarity <= 3, "rarity {} out of range", g.rarity);
            assert!(!g.boosts.is_empty());
            assert!(!g.name.is_empty());
            assert!(g.boosts.iter().all(|(_, n)| *n >= 1));
            assert!(!g.describe().is_empty());
        }
    }

    #[test]
    fn luck_and_threat_push_rarity_upward() {
        const N: usize = 4000;
        let mut plain = rng();
        let mut lucky = rng();
        let plain_total: usize = (0..N).map(|_| roll_gear(&mut plain, 0.0, 0.0).rarity).sum();
        let lucky_total: usize = (0..N).map(|_| roll_gear(&mut lucky, 0.3, 0.3).rarity).sum();
        assert!(
            lucky_total > plain_total,
            "luck {lucky_total} did not beat plain {plain_total}"
        );
    }

    #[test]
    fn legendary_stays_rare_even_at_absurd_luck() {
        const N: usize = 8000;
        // Unbounded luck used to push every tier roll past certainty, which
        // made every drop Legendary. The tier cap is what stops that.
        let mut rng = rng();
        let legendaries = (0..N)
            .filter(|_| roll_gear(&mut rng, 5.0, 5.0).rarity == 3)
            .count();
        let share = legendaries as f64 / N as f64;
        assert!(share < 0.12, "legendary share was {share}");
    }

    #[test]
    fn common_gear_remains_the_baseline_without_luck() {
        const N: usize = 8000;
        let mut rng = rng();
        let commons = (0..N)
            .filter(|_| roll_gear(&mut rng, 0.0, 0.0).rarity == 0)
            .count();
        let share = commons as f64 / N as f64;
        assert!((0.6..0.72).contains(&share), "common share was {share}");
    }

    #[test]
    fn gear_score_rewards_rarity_and_stacks() {
        let common = GearPiece {
            name: "a".into(),
            slot: GearSlot::Head,
            rarity: 0,
            boosts: vec![(StatBoost::Damage, 1)],
        };
        let legendary = GearPiece {
            name: "b".into(),
            slot: GearSlot::Head,
            rarity: 3,
            boosts: vec![(StatBoost::Damage, 1)],
        };
        assert!(legendary.score() > common.score());
    }

    #[test]
    fn equipping_stores_by_slot() {
        let mut e = Equipped::default();
        assert!(e.get(GearSlot::Head).is_none());
        e.set(GearPiece {
            name: "hat".into(),
            slot: GearSlot::Head,
            rarity: 1,
            boosts: vec![(StatBoost::Armor, 2)],
        });
        assert_eq!(e.get(GearSlot::Head).unwrap().name, "hat");
        assert!(e.get(GearSlot::Body).is_none());
        e.reset();
        assert!(e.get(GearSlot::Head).is_none());
    }

    // -- offers -------------------------------------------------------------

    #[test]
    fn an_offer_is_three_cards() {
        let mut rng = rng();
        let mut loadout = Loadout::default();
        loadout.reset();
        let boosts = AppliedBoosts::default();
        let cards = build_offer(&mut rng, &loadout, &boosts, EnvKind::Desk);
        assert_eq!(cards.len(), 3);
        assert!(cards.iter().all(|c| !c.title.is_empty()));
        assert!(cards.iter().all(|c| c.rarity <= 3));
    }

    #[test]
    fn an_offer_never_repeats_a_card() {
        let mut rng = rng();
        let mut loadout = Loadout::default();
        loadout.reset();
        let boosts = AppliedBoosts::default();
        for _ in 0..200 {
            let cards = build_offer(&mut rng, &loadout, &boosts, EnvKind::Desk);
            let mut titles: Vec<_> = cards.iter().map(|c| c.title.clone()).collect();
            titles.sort();
            let before = titles.len();
            titles.dedup();
            assert_eq!(titles.len(), before, "duplicate card in one offer");
        }
    }

    #[test]
    fn offers_keep_coming_once_every_weapon_is_mastered() {
        let mut rng = rng();
        let mut loadout = Loadout::default();
        loadout.reset();
        for kind in WeaponKind::ALL {
            loadout.add(kind);
            for _ in 0..MAX_LEVEL {
                loadout.level_up(kind);
            }
        }
        let boosts = AppliedBoosts::default();
        let cards = build_offer(&mut rng, &loadout, &boosts, EnvKind::Desk);
        assert_eq!(cards.len(), 3, "the pool must never run dry");
    }

    #[test]
    fn applying_a_stat_card_records_the_boost() {
        let mut stats = PlayerStats::default();
        let mut loadout = Loadout::default();
        let mut economy = Economy::default();
        let mut boosts = AppliedBoosts::default();
        let card = Card {
            title: "t".into(),
            detail: "d".into(),
            kind: CardKind::Stat(StatBoost::Damage),
            rarity: 0,
        };
        apply_card(&card, &mut stats, &mut loadout, &mut economy, &mut boosts);
        assert_eq!(boosts.entries, vec![(StatBoost::Damage, 1)]);
        apply_card(&card, &mut stats, &mut loadout, &mut economy, &mut boosts);
        assert_eq!(boosts.entries, vec![(StatBoost::Damage, 2)], "stacks");
    }

    #[test]
    fn resource_cards_pay_out_immediately() {
        let mut stats = PlayerStats::default();
        let mut loadout = Loadout::default();
        let mut economy = Economy::default();
        let mut boosts = AppliedBoosts::default();
        apply_card(
            &Card {
                title: "s".into(),
                detail: String::new(),
                kind: CardKind::FreeScrap(60.0),
                rarity: 0,
            },
            &mut stats,
            &mut loadout,
            &mut economy,
            &mut boosts,
        );
        assert!((economy.scrap - 60.0).abs() < 1e-6);
        apply_card(
            &Card {
                title: "c".into(),
                detail: String::new(),
                kind: CardKind::FreeCores(3.0),
                rarity: 0,
            },
            &mut stats,
            &mut loadout,
            &mut economy,
            &mut boosts,
        );
        assert!((economy.cores - 3.0).abs() < 1e-6);
    }

    #[test]
    fn weapon_cards_reach_the_loadout() {
        let mut stats = PlayerStats::default();
        let mut loadout = Loadout::default();
        loadout.reset();
        let mut economy = Economy::default();
        let mut boosts = AppliedBoosts::default();
        apply_card(
            &Card {
                title: "w".into(),
                detail: String::new(),
                kind: CardKind::NewWeapon(WeaponKind::Stapler),
                rarity: 2,
            },
            &mut stats,
            &mut loadout,
            &mut economy,
            &mut boosts,
        );
        assert_eq!(loadout.level_of(WeaponKind::Stapler), Some(1));
    }

    #[test]
    fn card_colours_are_defined_for_every_rarity() {
        for r in 0..=5 {
            let _ = card_color(r);
        }
    }
    #[test]
    fn later_levels_cost_meaningfully_more_than_early_ones() {
        // A flat-ish curve is what let a good build bank a level every twenty
        // seconds forever.
        let mut p = Progression::default();
        let first = p.to_next;
        for _ in 0..30 {
            let need = p.to_next;
            p.gain(need);
        }
        assert!(
            p.to_next > first * 8.0,
            "level {} still costs only {} against {first} at level 1",
            p.level,
            p.to_next
        );
    }

    #[test]
    fn a_fixed_xp_income_slows_down_as_the_run_goes_on() {
        // Levels per minute has to fall, or the card count runs away from
        // every curve it is measured against.
        let mut p = Progression::default();
        let income = 400.0;
        p.gain(income);
        let early = p.level;
        for _ in 0..12 {
            p.gain(income);
        }
        let total = p.level;
        let late_rate = f64::from(total - early) / 12.0;
        assert!(
            late_rate < f64::from(early - 1),
            "still gaining {late_rate} levels per batch after {total} levels"
        );
    }
    #[test]
    fn the_tree_offers_a_way_to_start_a_war() {
        // The one node that acts on the world rather than the player.
        let research = Research::default();
        let discord: Vec<_> = research.nodes.iter().filter(|n| n.discord > 0.0).collect();
        assert!(!discord.is_empty(), "no way to incite anybody");
        for node in &discord {
            assert!(node.endless, "a one-shot war node is a dead end late on");
            assert!(
                node.cost >= 10.0,
                "{} is too cheap to be a decision",
                node.title
            );
            assert_eq!(node.branch, Branch::Command);
        }
    }

    #[test]
    fn ordinary_research_does_not_start_wars() {
        let research = Research::default();
        let ordinary = research.nodes.iter().filter(|n| n.discord <= 0.0).count();
        assert!(ordinary > 10, "only {ordinary} plain nodes");
    }
}
