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
    /// Whether this boost does anything yet.
    ///
    /// Offering "+20% ally damage" a hundred seconds before allies exist is a
    /// dead card, and a tester's level-four offer was two of three dead.
    #[must_use]
    pub fn useful_yet(self, unlocks: &crate::onboarding::Unlocks) -> bool {
        match self {
            Self::AllyPower => unlocks.allies,
            Self::StructurePower | Self::BuildDiscount => unlocks.build,
            Self::CaptureRate => unlocks.territory,
            Self::Income => unlocks.build || unlocks.territory,
            _ => true,
        }
    }

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

    /// Skill points this rank costs on top of the Cores.
    ///
    /// Skill points arrive every third level and, until now, were counted,
    /// saved, reported and displayed while nothing on earth could spend them.
    /// They are the depth currency: Cores buy the flat percentages, which you
    /// can farm for, but the repeatable nodes and the two that reach out into
    /// the world are bought with levels. That makes them genuinely late without
    /// needing a clock check, and it gives levelling a second reward beyond the
    /// card.
    ///
    /// A rule rather than eighteen hand-authored numbers, so a new node lands
    /// in the right tier by being the kind of node it is.
    pub fn skill_cost(&self) -> u32 {
        if self.discord > 0.0 {
            return 2;
        }
        u32::from(self.endless)
    }

    /// Whether this rank is affordable right now.
    pub fn affordable(&self, cores: f32, skill_points: u32) -> bool {
        !self.maxed() && cores >= self.current_cost() && skill_points >= self.skill_cost()
    }

    /// Whether buying this rank would actually do anything.
    ///
    /// The two discord nodes start a war between the two strongest powers
    /// nearby, and if there is only one power in earshot there is no war to
    /// start. The purchase is irreversible, so it has to be refused rather than
    /// charged for: a strategist bought Whisper Campaign twice, paid
    /// twenty-eight Cores and four skill points, and got nothing either time.
    /// The failure was a hint that lasted a few seconds.
    pub fn effective(&self, a_war_is_possible: bool) -> bool {
        self.discord <= 0.0 || a_war_is_possible
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

/// Gear names, by slot, then rarity, then world.
///
/// Columns follow `EnvKind`: Desk, Forest, Rooftop, Grid, Arcane - the same
/// shape as the weapon and monster tables. There is no world-neutral way to
/// write these: a name is pure flavour, and a Paper Crown in the Arcane Sanctum
/// is exactly the complaint that started the theming work. So all sixty are
/// written out.
const GEAR_NAMES: [[[&str; EnvKind::COUNT]; 4]; 3] = [
    // HEAD
    [
        [
            "Paper Crown",
            "Leaf Cap",
            "Gutter Cap",
            "Static Cap",
            "Cloth Circlet",
        ],
        [
            "Bottle Cap Helm",
            "Acorn Helm",
            "Tin Helm",
            "Fuse Helm",
            "Bone Circlet",
        ],
        [
            "Thimble Helm",
            "Bark Crown",
            "Aerial Crown",
            "Node Crown",
            "Sigil Crown",
        ],
        [
            "Crown of Ink",
            "Crown of Thorns",
            "Crown of Antennae",
            "Crown of Circuits",
            "Crown of Ninefold Sight",
        ],
    ],
    // BODY
    [
        [
            "Tape Wrap",
            "Moss Wrap",
            "Tarp Wrap",
            "Cable Wrap",
            "Linen Wrap",
        ],
        [
            "Foil Vest",
            "Husk Vest",
            "Sheet Vest",
            "Mesh Vest",
            "Warded Vest",
        ],
        [
            "Cardboard Plate",
            "Bark Plate",
            "Ducting Plate",
            "Bus Plate",
            "Reliquary Plate",
        ],
        [
            "Mantle of Reams",
            "Mantle of Root",
            "Mantle of Girders",
            "Mantle of Lattices",
            "Mantle of Psalms",
        ],
    ],
    // TRINKET
    [
        [
            "Lucky Clip",
            "Lucky Pebble",
            "Lucky Washer",
            "Lucky Resistor",
            "Lucky Bead",
        ],
        [
            "Warm Battery",
            "Warm Seed",
            "Warm Coil",
            "Warm Capacitor",
            "Warm Reliquary",
        ],
        [
            "Compass Charm",
            "Dowsing Charm",
            "Weathervane Charm",
            "Beacon Charm",
            "Divining Charm",
        ],
        [
            "The Last Staple",
            "The Last Ember",
            "The Last Rivet",
            "The Last Fuse",
            "The Last Candle",
        ],
    ],
];

/// Roll a gear piece. `luck` and threat both push the rarity roll upward, which
/// is the main way the pacing dial pays out in power rather than numbers.
pub fn roll_gear(rng: &mut Rng, luck: f32, rarity_bonus: f32, world: EnvKind) -> GearPiece {
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
        name: GEAR_NAMES[slot as usize][rarity][world as usize].to_string(),
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
    unlocks: Res<crate::onboarding::Unlocks>,
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
    let cards = build_offer(&mut rng, &loadout, &boosts, *env, &unlocks);
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
    unlocks: &crate::onboarding::Unlocks,
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

    // How many of the interesting cards made it in. Refinements only appear
    // once weapons have run out, which is what "the pool is thin" means.
    let weapon_cards = pool.len();

    // Cards for systems that have not come online yet are dead on arrival.
    // A tester's level-four offer was two ally-and-structure cards out of
    // three, a hundred seconds before either existed.
    for boost in StatBoost::ALL {
        if !boost.useful_yet(unlocks) {
            continue;
        }
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

    // Refinements keep the offer meaningful once the interesting cards are
    // gone. The old test for "thin" was `pool.len() < 6`, which could never be
    // true: eighteen stat boosts and two economy cards are always in the pool,
    // so this branch was dead and the endless-progression promise in DESIGN.md
    // was unimplemented. Thinness is about *weapons* running out, which is the
    // thing that actually stops being offered.
    if weapon_cards == 0 {
        pool.push(Card {
            title: format!("Refinement {}", boosts.refinements + 1),
            detail: "+4% to damage, health, fire rate and income.".to_string(),
            kind: CardKind::Refinement,
            rarity: 2,
        });
    }

    // Weighted, not shuffled.
    //
    // A uniform draw over thirty cards gave the "level up a weapon" card a 10%
    // chance per level-up, so testers finished long runs with every weapon
    // still at level one - and a maxed weapon is nearly six times a level-one
    // one. The pool was actively pushing breadth over depth while the numbers
    // rewarded the opposite. `rarity` was decorative; it now decides how often
    // a card is seen.
    let mut offer = Vec::with_capacity(3);
    for _ in 0..3 {
        if pool.is_empty() {
            break;
        }
        let total: f32 = pool.iter().map(card_weight).sum();
        let mut roll = rng.range(0.0, total.max(f32::EPSILON));
        let mut picked = pool.len() - 1;
        for (i, card) in pool.iter().enumerate() {
            roll -= card_weight(card);
            if roll <= 0.0 {
                picked = i;
                break;
            }
        }
        offer.push(pool.swap_remove(picked));
    }
    offer
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
/// How much faster than the quickest monster the player may ever be.
///
/// Enough to disengage from anything, not enough to make contact impossible.
const MAX_SPEED_RATIO: f32 = 2.4;

/// How often a card should be seen, relative to the others.
///
/// Depth beats breadth in this game's own arithmetic, so the cards that deepen
/// a build are seen more often than the ones that widen it.
fn card_weight(card: &Card) -> f32 {
    match card.kind {
        // The strongest card in the game by a wide margin: a mastered weapon is
        // nearly six times a level-one one.
        CardKind::LevelWeapon(_) => 4.0,
        // A new weapon or a Refinement both widen rather than deepen, and are
        // worth about the same.
        CardKind::NewWeapon(_) | CardKind::Refinement => 2.0,
        CardKind::Stat(_) => 1.0,
        // A one-off lump of currency is the weakest thing in the pool.
        CardKind::FreeScrap(_) | CardKind::FreeCores(_) => 0.8,
    }
}

/// Health multiplier from levels alone.
#[must_use]
pub fn constitution(level: u32) -> f32 {
    1.0 + f32::from(u16::try_from(level.saturating_sub(1)).unwrap_or(u16::MAX)) * 0.04
}

fn recompute_stats(
    mut events: MessageReader<RecomputeStats>,
    progression: Res<Progression>,
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

    // Constitution: a little more health for every level taken, on top of
    // whatever cards and research have added.
    //
    // Without it, health is the one stat the difficulty curve does not have to
    // beat - enemies scale with the clock and with your level, and the pool
    // they are chewing through only grows if the card offer happens to include
    // one of two health cards. Playtesting found a level-6 character on 34
    // health facing eighty-four monsters, which is not a difficulty curve, it
    // is a wall. This is deliberately smaller than a health card so taking one
    // still matters.
    next.max_hp *= constitution(progression.level);

    // Cap the run away from the horde.
    //
    // MoveSpeed is a plain 8% multiplier with no ceiling, and enemy speeds are
    // constants that nothing scales. A sweep reached 15.5 against a fastest
    // enemy of 4.6 and finished with four thousand monsters alive and one of
    // them within twelve metres - the late game had no failure state, because
    // nothing could reach the player. Staying comfortably faster than the
    // horde is the point of the stat; being untouchable is not.
    let ceiling = crate::enemy::fastest_enemy_speed() * MAX_SPEED_RATIO;
    next.move_speed = next.move_speed.min(ceiling);

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
    use std::ops::Not as _;

    use super::*;

    /// Every system online, which is what a card-pool test wants unless it is
    /// specifically about the unlock gate.
    fn everything_unlocked() -> crate::onboarding::Unlocks {
        crate::onboarding::Unlocks {
            build: true,
            territory: true,
            allies: true,
            research: true,
            threat_dial: true,
            ..crate::onboarding::Unlocks::default()
        }
    }

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
    fn a_war_node_is_not_sold_when_there_is_no_war_to_start() {
        // Twenty-eight Cores and four skill points went on two purchases that
        // did nothing, because the failure was a hint and the spend was not.
        let nodes = default_nodes();
        let discord = nodes.iter().find(|n| n.discord > 0.0).expect("no discord");
        assert!(!discord.effective(false), "sold a war with nobody to turn");
        assert!(discord.effective(true));
    }

    #[test]
    fn every_other_node_is_always_worth_buying() {
        // Only the discord nodes depend on the world; a percentage does not.
        for node in default_nodes().iter().filter(|n| n.discord <= 0.0) {
            assert!(node.effective(false), "{} was gated", node.title);
        }
    }

    #[test]
    fn gear_is_named_for_the_world_it_drops_in() {
        // A Paper Crown in the Arcane Sanctum is the complaint that started the
        // theming work. Every slot, every rarity, every world.
        for (slot, rarities) in GEAR_NAMES.iter().enumerate() {
            for (rarity, row) in rarities.iter().enumerate() {
                let unique: std::collections::BTreeSet<_> = row.iter().collect();
                assert_eq!(
                    unique.len(),
                    EnvKind::COUNT,
                    "slot {slot} rarity {rarity} reuses a name across worlds: {row:?}"
                );
            }
        }
    }

    #[test]
    fn a_drop_takes_the_name_of_the_world_it_fell_in() {
        let mut a = Rng::seeded(0x9001);
        let mut b = Rng::seeded(0x9001);
        let desk = roll_gear(&mut a, 0.0, 0.0, EnvKind::Desk);
        let sanctum = roll_gear(&mut b, 0.0, 0.0, EnvKind::Arcane);
        // Same seed, so the same slot and rarity - only the name should differ.
        assert_eq!(desk.slot, sanctum.slot);
        assert_eq!(desk.rarity, sanctum.rarity);
        assert_ne!(desk.name, sanctum.name, "{} in the Sanctum", desk.name);
    }

    #[test]
    fn skill_points_have_something_to_buy() {
        // They accumulated, saved, loaded and displayed while nothing in the
        // game could spend them.
        let nodes = default_nodes();
        assert!(
            nodes.iter().any(|n| n.skill_cost() > 0),
            "no node costs a skill point, so they are still unspendable"
        );
    }

    #[test]
    fn the_flat_percentages_cost_only_cores() {
        // Skill points are the depth currency. Gating the entry-level nodes
        // behind them would stall the tree for the first nine levels.
        let nodes = default_nodes();
        let flat = nodes
            .iter()
            .filter(|n| !n.endless && n.discord == 0.0)
            .count();
        assert!(flat >= 10, "only {flat} flat nodes");
        for node in nodes.iter().filter(|n| !n.endless && n.discord == 0.0) {
            assert_eq!(node.skill_cost(), 0, "{} is gated", node.title);
        }
    }

    #[test]
    fn reaching_out_into_the_world_costs_more_than_repeating_yourself() {
        let nodes = default_nodes();
        let discord = nodes.iter().find(|n| n.discord > 0.0).expect("no discord");
        let endless = nodes
            .iter()
            .find(|n| n.endless && n.discord == 0.0)
            .expect("no endless");
        assert!(discord.skill_cost() > endless.skill_cost());
    }

    #[test]
    fn a_node_needs_both_currencies() {
        let node = default_nodes()
            .into_iter()
            .find(|n| n.discord > 0.0)
            .expect("no discord node");
        let cost = node.current_cost();
        assert!(!node.affordable(cost - 1.0, 99), "bought without the cores");
        assert!(!node.affordable(9999.0, 1), "bought without the points");
        assert!(node.affordable(cost, node.skill_cost()));
    }

    #[test]
    fn a_maxed_node_is_never_affordable() {
        // The purchase path checks affordability instead of maxed-ness, so a
        // maxed node reporting affordable would sell infinite ranks.
        let mut node = default_nodes()
            .into_iter()
            .find(|n| !n.endless)
            .expect("no capped node");
        node.rank = node.max_rank;
        assert!(node.affordable(9999.0, 99).not());
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
            let g = roll_gear(&mut rng, 0.0, 0.0, EnvKind::Desk);
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
        let plain_total: usize = (0..N)
            .map(|_| roll_gear(&mut plain, 0.0, 0.0, EnvKind::Desk).rarity)
            .sum();
        let lucky_total: usize = (0..N)
            .map(|_| roll_gear(&mut lucky, 0.3, 0.3, EnvKind::Desk).rarity)
            .sum();
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
            .filter(|_| roll_gear(&mut rng, 5.0, 5.0, EnvKind::Desk).rarity == 3)
            .count();
        let share = legendaries as f64 / N as f64;
        assert!(share < 0.12, "legendary share was {share}");
    }

    #[test]
    fn common_gear_remains_the_baseline_without_luck() {
        const N: usize = 8000;
        let mut rng = rng();
        let commons = (0..N)
            .filter(|_| roll_gear(&mut rng, 0.0, 0.0, EnvKind::Desk).rarity == 0)
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
        let cards = build_offer(
            &mut rng,
            &loadout,
            &boosts,
            EnvKind::Desk,
            &everything_unlocked(),
        );
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
            let cards = build_offer(
                &mut rng,
                &loadout,
                &boosts,
                EnvKind::Desk,
                &everything_unlocked(),
            );
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
        let cards = build_offer(
            &mut rng,
            &loadout,
            &boosts,
            EnvKind::Desk,
            &everything_unlocked(),
        );
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
    #[test]
    fn health_grows_with_level_but_less_than_a_card_would() {
        // Enemies scale with the clock and with the player's level; if health
        // only moves when the offer happens to contain one of two cards, the
        // curve has nothing to beat.
        assert!(
            (constitution(1) - 1.0).abs() < 1e-6,
            "level one is the baseline"
        );
        assert!(constitution(10) > constitution(1));
        assert!(constitution(30) > constitution(10));

        // A MaxHp card is +22 flat on a 120 base, about 18%. Four levels of
        // constitution should still be worth less than one card.
        let four_levels = (constitution(5) - 1.0) * 120.0;
        assert!(
            four_levels < 22.0,
            "levels outpace the card at {four_levels}"
        );
    }

    #[test]
    fn constitution_never_shrinks_the_pool_or_overflows() {
        let mut last = 0.0;
        for level in 1..=200 {
            let now = constitution(level);
            assert!(now >= last && now.is_finite(), "broke at level {level}");
            last = now;
        }
        assert!(constitution(u32::MAX).is_finite());
    }
    #[test]
    fn move_speed_is_capped_against_the_fastest_monster() {
        // Uncapped, this stat ends the game: a player fast enough that nothing
        // can touch them turns a horde of thousands into scenery.
        let ceiling = crate::enemy::fastest_enemy_speed() * MAX_SPEED_RATIO;
        assert!(
            ceiling > crate::player::BASE_SPEED,
            "the cap bites immediately"
        );
        assert!(
            ceiling < crate::enemy::fastest_enemy_speed() * 3.0,
            "a threefold lead is what made the late game inert"
        );
    }
    #[test]
    fn the_offer_favours_deepening_a_build_over_widening_it() {
        // A uniform draw gave the level-a-weapon card a 10% chance, so testers
        // finished long runs with every weapon still at level one - while a
        // maxed weapon is nearly six times a level-one one. The pool was
        // pushing the opposite of what the numbers reward.
        let mut rng = Rng::seeded(9);
        let mut loadout = Loadout::default();
        loadout.reset();
        loadout.add(WeaponKind::Stapler);
        let boosts = AppliedBoosts::default();

        let mut level_cards = 0;
        let mut draws = 0;
        for _ in 0..300 {
            for card in build_offer(
                &mut rng,
                &loadout,
                &boosts,
                EnvKind::Desk,
                &everything_unlocked(),
            ) {
                draws += 1;
                if matches!(card.kind, CardKind::LevelWeapon(_)) {
                    level_cards += 1;
                }
            }
        }
        let share = f64::from(level_cards) / f64::from(draws);
        assert!(
            share > 0.12,
            "only {share:.3} of cards deepened a build; uniform was ~0.07"
        );
    }

    #[test]
    fn cards_for_locked_systems_are_not_offered() {
        // A tester's level-four offer was two ally-and-structure cards out of
        // three, a hundred seconds before either system existed.
        let mut rng = Rng::seeded(4);
        let mut loadout = Loadout::default();
        loadout.reset();
        let boosts = AppliedBoosts::default();
        let nothing = crate::onboarding::Unlocks::default();

        for _ in 0..200 {
            for card in build_offer(&mut rng, &loadout, &boosts, EnvKind::Desk, &nothing) {
                if let CardKind::Stat(boost) = card.kind {
                    assert!(
                        boost.useful_yet(&nothing),
                        "{} offered with nothing unlocked",
                        card.title
                    );
                }
            }
        }
    }

    #[test]
    fn refinements_appear_once_the_weapons_run_out() {
        // The old condition was `pool.len() < 6`, which eighteen stat cards
        // made permanently false, so the endless-progression promise in
        // DESIGN.md was never implemented.
        let mut rng = Rng::seeded(11);
        let mut loadout = Loadout::default();
        loadout.reset();
        for kind in WeaponKind::ALL {
            loadout.add(kind);
            for _ in 0..12 {
                loadout.level_up(kind);
            }
        }
        let boosts = AppliedBoosts::default();

        let mut seen = false;
        for _ in 0..80 {
            if build_offer(
                &mut rng,
                &loadout,
                &boosts,
                EnvKind::Desk,
                &everything_unlocked(),
            )
            .iter()
            .any(|c| matches!(c.kind, CardKind::Refinement))
            {
                seen = true;
                break;
            }
        }
        assert!(seen, "a fully mastered loadout never gets a Refinement");
    }
}
