//! The game's colour vocabulary.
//!
//! One warm desk lamp in a dark office: everything reads as either warm wood
//! and stationery, cold screen-glow, or the acid tones reserved for hostiles
//! and pickups so the player can parse a crowded field instantly.

use bevy::prelude::*;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

// -- the desk itself --------------------------------------------------------
pub const DESK_WOOD: Color = rgb(122, 80, 48);
pub const DESK_WOOD_DARK: Color = rgb(92, 58, 33);
pub const DESK_EDGE: Color = rgb(70, 44, 26);
pub const MOUSEPAD: Color = rgb(34, 38, 48);
pub const MOUSEPAD_TRIM: Color = rgb(58, 66, 84);
pub const PAPER: Color = rgb(238, 236, 228);

// -- stationery -------------------------------------------------------------
pub const PLASTIC_DARK: Color = rgb(38, 40, 46);
pub const PLASTIC_MID: Color = rgb(64, 68, 78);
pub const KEYCAP: Color = rgb(226, 226, 230);
pub const METAL: Color = rgb(176, 180, 190);
pub const METAL_DARK: Color = rgb(110, 116, 126);
pub const CERAMIC: Color = rgb(242, 240, 236);
pub const COFFEE: Color = rgb(58, 32, 18);
pub const PENCIL_YELLOW: Color = rgb(240, 186, 44);
pub const ERASER_PINK: Color = rgb(236, 138, 150);
pub const GRAPHITE: Color = rgb(48, 48, 54);
pub const STICKY_YELLOW: Color = rgb(250, 220, 90);
pub const STICKY_PINK: Color = rgb(246, 148, 178);
pub const STICKY_CYAN: Color = rgb(126, 220, 232);
pub const RUBBER_BAND: Color = rgb(206, 118, 92);
pub const CORK: Color = rgb(196, 152, 96);
pub const LEAF: Color = rgb(84, 150, 78);
pub const TERRACOTTA: Color = rgb(178, 96, 66);

// -- light ------------------------------------------------------------------
pub const LAMP_SHADE: Color = rgb(216, 92, 62);
pub const LAMP_GLOW: Color = rgb(255, 214, 148);
pub const SCREEN_GLOW: Color = rgb(96, 186, 240);
pub const SCREEN_DIM: Color = rgb(24, 44, 66);

// -- the player -------------------------------------------------------------
pub const DUCK_BODY: Color = rgb(255, 208, 62);
pub const DUCK_SHADE: Color = rgb(228, 168, 40);
pub const DUCK_BEAK: Color = rgb(246, 132, 44);
pub const DUCK_EYE: Color = rgb(28, 26, 32);

// -- hostiles ---------------------------------------------------------------
pub const DUST_GREY: Color = rgb(148, 142, 138);
pub const DUST_DARK: Color = rgb(102, 98, 96);
pub const ANT_BODY: Color = rgb(78, 40, 30);
pub const CLIP_STEEL: Color = rgb(190, 194, 204);
pub const STAPLE_STEEL: Color = rgb(214, 218, 226);
pub const CRUMB_TAN: Color = rgb(198, 156, 92);
pub const TACK_RED: Color = rgb(206, 62, 58);
pub const SLIME_BROWN: Color = rgb(96, 58, 34);
pub const MOTH_WING: Color = rgb(180, 168, 148);
pub const GREMLIN_TEAL: Color = rgb(64, 196, 176);
/// Reserved for elites, and not any faction's colour.
///
/// This was (226, 74, 200), which is the BLOOM faction's magenta almost exactly,
/// so "the purple circle" meant either "an elite" or "belongs to BLOOM" with no
/// way to tell which.
pub const ELITE_TRIM: Color = rgb(120, 246, 255);
/// And this was close to the RUST faction's red. Boss rings are now white-hot,
/// which no faction is and nothing else in the game uses.
pub const BOSS_TRIM: Color = rgb(255, 246, 214);

// -- feedback and UI --------------------------------------------------------
pub const XP_GREEN: Color = rgb(126, 232, 128);
pub const HEAL_RED: Color = rgb(240, 86, 96);
pub const GEAR_GOLD: Color = rgb(250, 194, 84);
pub const DANGER: Color = rgb(232, 72, 66);
/// The ring under the player themselves.
///
/// Its own colour, and not `ALLY_TRIM`: a UX pass established that the player was
/// the only gameplay entity in the game with no floor marker at all - allies,
/// turrets, elites, bosses, nests, forts and zones every one had one - and that
/// the duck's own gold is the same gold as its shots, its gear pickups, the
/// neutral zone ring and the HUD accent. Measured 2.25:1 against the desk floor,
/// where 3:1 is the floor for a graphic that means something. On the rooftop it
/// could not be found at all.
pub const HERO_TRIM: Color = rgb(255, 255, 255);

/// The ring under anything that belongs to the player. Their faction green, but
/// brighter, because "where are my allies" was a question the screen could not
/// answer at all.
pub const ALLY_TRIM: Color = rgb(120, 255, 158);

pub const HUD_TEXT: Color = rgb(238, 240, 246);
pub const HUD_DIM: Color = rgb(150, 156, 172);
pub const HUD_PANEL: Color = Color::srgba(0.05, 0.055, 0.075, 0.88);
pub const HUD_PANEL_SOLID: Color = rgb(14, 15, 20);
pub const ACCENT: Color = rgb(255, 190, 74);

/// Rarity tint, shared by gear drops and upgrade cards so the two systems read
/// as one economy.
pub const RARITY: [Color; 4] = [
    rgb(176, 182, 196), // Common
    rgb(96, 190, 236),  // Rare
    rgb(186, 124, 246), // Epic
    rgb(252, 172, 66),  // Legendary
];

pub fn rarity_name(tier: usize) -> &'static str {
    match tier {
        0 => "Common",
        1 => "Rare",
        2 => "Epic",
        _ => "Legendary",
    }
}

/// Lighten/darken a colour in linear space, for hover states and shading.
pub fn shade(c: Color, factor: f32) -> Color {
    let l = c.to_linear();
    Color::LinearRgba(LinearRgba {
        red: l.red * factor,
        green: l.green * factor,
        blue: l.blue * factor,
        alpha: l.alpha,
    })
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    c.with_alpha(a)
}
