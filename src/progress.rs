//! Levels, upgrade cards, research, and gear.
//!
//! The run is endless, so nothing here has a ceiling. The card pool eventually
//! runs dry of new content and starts offering Refinements; the research tree
//! ends in repeatable nodes with rising costs; weapons cap at eight and then
//! evolve. The curve never flattens, it just changes what it is made of.

use bevy::prelude::*;

use crate::allies::{AllyKind, Economy};
use crate::palette as pal;
use crate::player::PlayerStats;
use crate::rng::Rng;
use crate::weapons::{Loadout, MAX_LEVEL, WeaponKind};
use crate::{AppState, GameSet, RunSetup};

// -- experience -------------------------------------------------------------

#[derive(Resource)]
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
        while self.xp >= self.to_next {
            self.xp -= self.to_next;
            self.level += 1;
            self.pending_levels += 1;
            // Every third level also funds the research tree.
            if self.level % 3 == 0 {
                self.skill_points += 1;
            }
            // Superlinear but gentle: level 30 costs roughly 12x level 1.
            self.to_next = 12.0 * (1.0 + self.level as f32 * 0.34).powf(1.12);
        }
    }

    pub fn fraction(&self) -> f32 {
        (self.xp / self.to_next).clamp(0.0, 1.0)
    }
}

// -- upgrade cards ----------------------------------------------------------

#[derive(Clone)]
pub struct Card {
    pub title: String,
    pub detail: String,
    pub kind: CardKind,
    pub rarity: usize,
}

#[derive(Clone, Copy)]
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
    const ALL: [Self; 18] = [
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
#[derive(Resource, Default)]
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

#[derive(Clone)]
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
}

impl ResearchNode {
    pub fn current_cost(&self) -> f32 {
        self.cost * (1.0 + self.rank as f32 * if self.endless { 0.65 } else { 0.4 })
    }

    pub fn maxed(&self) -> bool {
        !self.endless && self.rank >= self.max_rank
    }
}

#[derive(Resource)]
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
    };
    vec![
        // MIGHT
        n(Might, "Honed Edge", "+10% weapon damage", 5, 3.0, StatBoost::Damage, false),
        n(Might, "Weak Points", "+5% critical chance", 4, 4.0, StatBoost::Crit, false),
        n(Might, "Broad Swing", "+10% area", 4, 4.0, StatBoost::Area, false),
        n(Might, "Endless Edge", "+8% damage, forever", 0, 8.0, StatBoost::Damage, true),
        // SWIFT
        n(Swift, "Quick Hands", "+10% fire rate", 5, 3.0, StatBoost::Haste, false),
        n(Swift, "Light Step", "+6% move speed", 4, 3.0, StatBoost::MoveSpeed, false),
        n(Swift, "Extra Barrel", "+1 projectile", 2, 9.0, StatBoost::ProjectileCount, false),
        n(Swift, "Endless Tempo", "+7% fire rate, forever", 0, 8.0, StatBoost::Haste, true),
        // VITAL
        n(Vital, "Hard Shell", "+20 max health", 5, 3.0, StatBoost::MaxHp, false),
        n(Vital, "Plating", "+2 armour", 4, 4.0, StatBoost::Armor, false),
        n(Vital, "Recovery", "+0.6 regen", 4, 4.0, StatBoost::Regen, false),
        n(Vital, "Endless Vigour", "+18 health, forever", 0, 7.0, StatBoost::MaxHp, true),
        // COMMAND
        n(Command, "Drill", "+15% ally power", 5, 3.0, StatBoost::AllyPower, false),
        n(Command, "Machining", "+15% structure power", 5, 3.0, StatBoost::StructurePower, false),
        n(Command, "Supply Lines", "+15% income", 4, 4.0, StatBoost::Income, false),
        n(Command, "Flag Bearer", "+25% capture speed", 3, 5.0, StatBoost::CaptureRate, false),
        n(Command, "Endless Logistics", "+12% income, forever", 0, 8.0, StatBoost::Income, true),
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

#[derive(Clone)]
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

#[derive(Resource, Default)]
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

    fn set(&mut self, piece: GearPiece) {
        match piece.slot {
            GearSlot::Head => self.head = Some(piece),
            GearSlot::Body => self.body = Some(piece),
            GearSlot::Trinket => self.trinket = Some(piece),
        }
    }
}

const GEAR_NAMES: [[&str; 4]; 3] = [
    ["Paper Crown", "Bottle Cap Helm", "Thimble Helm", "Crown of Ink"],
    ["Tape Wrap", "Foil Vest", "Cardboard Plate", "Mantle of Reams"],
    ["Lucky Clip", "Warm Battery", "Compass Charm", "The Last Staple"],
];

/// Roll a gear piece. `luck` and threat both push the rarity roll upward, which
/// is the main way the pacing dial pays out in power rather than numbers.
pub fn roll_gear(rng: &mut Rng, luck: f32, rarity_bonus: f32) -> GearPiece {
    let slot = GearSlot::ALL[rng.below(3)];
    let mut rarity = 0;
    let mut chance = 0.34 + luck + rarity_bonus;
    while rarity < 3 && rng.chance(chance) {
        rarity += 1;
        // Each successive upgrade is harder, so Legendary stays rare even at
        // absurd luck.
        chance *= 0.42;
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
#[derive(Message)]
pub struct RecomputeStats;

/// Applied bonuses that persist for the run, accumulated from cards.
#[derive(Resource, Default)]
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
            .add_systems(OnExit(AppState::Menu), reset_progress.in_set(RunSetup::Reset));
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
    let cards = build_offer(&mut rng, &loadout, &boosts);
    commands.insert_resource(CardOffer {
        cards,
        reroll_available: true,
    });
    next.set(AppState::LevelUp);
}

/// Three distinct options, weighted so a new weapon is exciting but not
/// guaranteed, and never offering the same thing twice.
pub fn build_offer(rng: &mut Rng, loadout: &Loadout, boosts: &AppliedBoosts) -> Vec<Card> {
    let mut pool: Vec<Card> = Vec::new();

    for kind in loadout.offerable() {
        match loadout.level_of(kind) {
            None => pool.push(Card {
                title: kind.name().to_string(),
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
                    title: format!("{} +", kind.name()),
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
pub fn recruit_hint(economy: &Economy) -> String {
    AllyKind::ALL
        .iter()
        .map(|k| {
            let affordable = economy.cores >= k.core_cost();
            format!(
                "{}{} {}",
                if affordable { "" } else { "-" },
                k.name(),
                k.core_cost() as u32
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}
