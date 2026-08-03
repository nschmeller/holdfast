//! Sound, synthesized at startup.
//!
//! There are no audio files. Each effect is a few hundred milliseconds of PCM
//! generated from oscillators and noise, wrapped in a WAV header and handed to
//! Bevy as an `AudioSource`. The whole sound bank costs well under a megabyte
//! of RAM and nothing at all in download size, which matters a great deal for
//! a web build.

use bevy::audio::{GlobalVolume, PlaybackMode, Volume};
use bevy::prelude::*;

use crate::common::SfxEvent;

const SAMPLE_RATE: u32 = 22_050;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Sfx {
    Dart,
    Sweep,
    Band,
    Stapler,
    Beam,
    Laser,
    Nova,
    Fan,
    Kill,
    BossDown,
    PlayerHurt,
    Death,
    Pickup,
    Core,
    Heal,
    Gear,
    LevelUp,
    Build,
    Place,
    Recruit,
    Order,
    Capture,
    Lost,
    Surge,
    Tick,
    Deny,
    WaveStart,
}

impl Sfx {
    const ALL: [Self; 27] = [
        Self::Dart,
        Self::Sweep,
        Self::Band,
        Self::Stapler,
        Self::Beam,
        Self::Laser,
        Self::Nova,
        Self::Fan,
        Self::Kill,
        Self::BossDown,
        Self::PlayerHurt,
        Self::Death,
        Self::Pickup,
        Self::Core,
        Self::Heal,
        Self::Gear,
        Self::LevelUp,
        Self::Build,
        Self::Place,
        Self::Recruit,
        Self::Order,
        Self::Capture,
        Self::Lost,
        Self::Surge,
        Self::Tick,
        Self::Deny,
        Self::WaveStart,
    ];

    /// Base volume, so the mix is balanced at the source rather than at every
    /// call site.
    fn gain(self) -> f32 {
        match self {
            Self::Dart | Self::Tick => 0.16,
            Self::Kill | Self::Pickup => 0.2,
            Self::Sweep | Self::Band | Self::Beam | Self::Fan => 0.24,
            Self::Stapler | Self::Laser | Self::Place | Self::Order => 0.28,
            Self::Heal | Self::Core | Self::Deny => 0.3,
            Self::Build | Self::Recruit | Self::Capture | Self::Lost => 0.34,
            Self::PlayerHurt | Self::Gear | Self::WaveStart => 0.4,
            Self::Nova | Self::LevelUp | Self::Surge => 0.45,
            Self::BossDown | Self::Death => 0.5,
        }
    }
}

/// The generated bank, indexed by `Sfx as usize`.
#[derive(Debug, Resource)]
pub struct SoundBank {
    clips: Vec<Handle<AudioSource>>,
}

impl SoundBank {
    fn get(&self, sfx: Sfx) -> Handle<AudioSource> {
        self.clips[sfx as usize].clone()
    }
}

/// A one-shot sound entity, despawned once it has finished.
#[derive(Component)]
struct OneShot(f32);

#[derive(Debug)]
pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        // Silent by default under the pilot bridge. Several unattended instances
        // on one desk is a lot of noise, nobody is listening to an agent's game,
        // and the person whose desk it is should not have to ask twice.
        // `HOLDFAST_MUTE=0` turns the sound back on if a run is ever worth
        // hearing.
        let truthy = |v: String| {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false")
        };
        let start_muted = match std::env::var("HOLDFAST_MUTE") {
            Ok(v) => truthy(v),
            Err(_) => std::env::var("HOLDFAST_PILOT").is_ok(),
        };
        app.insert_resource(Muted(start_muted))
            .add_systems(Startup, build_bank)
            .add_systems(Update, (toggle_mute, play_sfx, reap_one_shots));
        if start_muted {
            app.insert_resource(GlobalVolume::new(Volume::Linear(0.0)));
        }
    }
}

/// Whether the game is silent. `M` toggles it, `HOLDFAST_MUTE=1` starts that way.
#[derive(Resource, Debug, Default)]
pub struct Muted(pub bool);

/// `M` mutes and unmutes.
///
/// Not gated on any state, so it works at the menu, mid-run and on the results
/// screen - a mute you have to be in the right screen to reach is not a mute.
fn toggle_mute(
    keys: Res<ButtonInput<KeyCode>>,
    mut muted: ResMut<Muted>,
    mut volume: ResMut<GlobalVolume>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    muted.0 = !muted.0;
    volume.volume = Volume::Linear(if muted.0 { 0.0 } else { 1.0 });
}

// -- synthesis --------------------------------------------------------------

/// A tiny mono synth. Everything is written into `buf` in `[-1, 1]`.
struct Synth {
    buf: Vec<f32>,
}

impl Synth {
    fn new(seconds: f32) -> Self {
        Self {
            buf: vec![0.0; (seconds * SAMPLE_RATE as f32) as usize],
        }
    }

    /// Additive tone with a linear frequency sweep and exponential decay.
    fn tone(&mut self, start_hz: f32, end_hz: f32, amp: f32, decay: f32, wave: Wave) -> &mut Self {
        let n = self.buf.len();
        let mut phase = 0.0f32;
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            let progress = i as f32 / n as f32;
            let hz = start_hz + (end_hz - start_hz) * progress;
            phase += hz / SAMPLE_RATE as f32;
            phase -= phase.floor();
            let env = (-t * decay).exp();
            self.buf[i] += wave.sample(phase) * amp * env;
        }
        self
    }

    /// Filtered noise, for impacts and wind.
    fn noise(&mut self, amp: f32, decay: f32, smoothing: f32, seed: u64) -> &mut Self {
        let mut rng = crate::rng::Rng::seeded(seed);
        let mut last = 0.0f32;
        let n = self.buf.len();
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            let white = rng.range(-1.0, 1.0);
            // One-pole low pass: `smoothing` near 1 is dull, near 0 is bright.
            last = last * smoothing + white * (1.0 - smoothing);
            let env = (-t * decay).exp();
            self.buf[i] += last * amp * env;
        }
        self
    }

    /// Sequence of short pitched blips, for fanfares and confirmations.
    fn arpeggio(&mut self, notes: &[f32], note_len: f32, amp: f32, wave: Wave) -> &mut Self {
        for (k, hz) in notes.iter().enumerate() {
            let start = (k as f32 * note_len * SAMPLE_RATE as f32) as usize;
            let count = (note_len * SAMPLE_RATE as f32) as usize;
            let mut phase = 0.0f32;
            for i in 0..count {
                let idx = start + i;
                if idx >= self.buf.len() {
                    break;
                }
                let t = i as f32 / SAMPLE_RATE as f32;
                phase += hz / SAMPLE_RATE as f32;
                phase -= phase.floor();
                // Short attack avoids the click a raw square start produces.
                let attack = (t * 220.0).min(1.0);
                let env = attack * (-t * 7.0).exp();
                self.buf[idx] += wave.sample(phase) * amp * env;
            }
        }
        self
    }

    /// Fade the last few milliseconds so nothing ends on a discontinuity.
    fn finish(mut self) -> Vec<u8> {
        let n = self.buf.len();
        let tail = (SAMPLE_RATE as f32 * 0.008) as usize;
        for i in 0..tail.min(n) {
            let f = i as f32 / tail as f32;
            self.buf[n - 1 - i] *= f;
        }
        encode_wav(&self.buf)
    }
}

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Saw,
    Triangle,
}

impl Wave {
    fn sample(self, phase: f32) -> f32 {
        use std::f32::consts::TAU;
        match self {
            Self::Sine => (phase * TAU).sin(),
            Self::Square => {
                if phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Self::Saw => phase * 2.0 - 1.0,
            Self::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        }
    }
}

/// Wrap PCM samples in a 16-bit mono WAV container.
fn encode_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // format: PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // channels: mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in samples {
        // Soft clip rather than hard: layered tones routinely exceed 1.0 and
        // tanh keeps that as warmth instead of crackle.
        let clipped = s.tanh();
        out.extend_from_slice(&((clipped * 32_000.0) as i16).to_le_bytes());
    }

    out
}

fn render(sfx: Sfx) -> Vec<u8> {
    use Wave::{Saw, Sine, Square, Triangle};
    match sfx {
        Sfx::Dart => {
            let mut s = Synth::new(0.09);
            s.tone(900.0, 420.0, 0.5, 34.0, Square);
            s.noise(0.14, 60.0, 0.6, 1);
            s.finish()
        }
        Sfx::Sweep => {
            let mut s = Synth::new(0.22);
            s.noise(0.55, 16.0, 0.82, 2);
            s.tone(220.0, 620.0, 0.2, 12.0, Triangle);
            s.finish()
        }
        Sfx::Band => {
            let mut s = Synth::new(0.16);
            s.tone(300.0, 900.0, 0.42, 20.0, Triangle);
            s.finish()
        }
        Sfx::Stapler => {
            let mut s = Synth::new(0.16);
            s.noise(0.6, 28.0, 0.42, 3);
            s.tone(180.0, 90.0, 0.4, 26.0, Square);
            s.finish()
        }
        Sfx::Beam => {
            let mut s = Synth::new(0.2);
            s.tone(1400.0, 1900.0, 0.3, 14.0, Sine);
            s.tone(700.0, 950.0, 0.2, 14.0, Sine);
            s.finish()
        }
        Sfx::Laser => {
            let mut s = Synth::new(0.26);
            s.tone(2200.0, 300.0, 0.42, 15.0, Saw);
            s.noise(0.16, 30.0, 0.3, 4);
            s.finish()
        }
        Sfx::Nova => {
            let mut s = Synth::new(0.65);
            s.noise(0.8, 7.0, 0.9, 5);
            s.tone(160.0, 40.0, 0.55, 6.0, Sine);
            s.finish()
        }
        Sfx::Fan => {
            let mut s = Synth::new(0.34);
            s.noise(0.7, 9.0, 0.93, 6);
            s.finish()
        }
        Sfx::Kill => {
            let mut s = Synth::new(0.11);
            s.noise(0.5, 42.0, 0.55, 7);
            s.tone(420.0, 140.0, 0.3, 40.0, Triangle);
            s.finish()
        }
        Sfx::BossDown => {
            let mut s = Synth::new(1.3);
            s.noise(0.85, 4.0, 0.95, 8);
            s.tone(180.0, 30.0, 0.7, 3.2, Sine);
            s.arpeggio(&[392.0, 330.0, 262.0, 196.0], 0.16, 0.3, Triangle);
            s.finish()
        }
        Sfx::PlayerHurt => {
            let mut s = Synth::new(0.24);
            s.tone(300.0, 90.0, 0.6, 16.0, Saw);
            s.noise(0.3, 22.0, 0.7, 9);
            s.finish()
        }
        Sfx::Death => {
            let mut s = Synth::new(1.5);
            s.tone(420.0, 55.0, 0.6, 2.6, Saw);
            s.noise(0.4, 3.0, 0.92, 10);
            s.finish()
        }
        Sfx::Pickup => {
            let mut s = Synth::new(0.09);
            s.tone(1100.0, 1600.0, 0.34, 30.0, Sine);
            s.finish()
        }
        Sfx::Core => {
            let mut s = Synth::new(0.3);
            s.arpeggio(&[784.0, 1046.0], 0.1, 0.32, Sine);
            s.finish()
        }
        Sfx::Heal => {
            let mut s = Synth::new(0.36);
            s.arpeggio(&[523.0, 659.0, 784.0], 0.1, 0.3, Sine);
            s.finish()
        }
        Sfx::Gear => {
            let mut s = Synth::new(0.6);
            s.arpeggio(&[523.0, 659.0, 784.0, 1046.0], 0.13, 0.34, Triangle);
            s.finish()
        }
        Sfx::LevelUp => {
            let mut s = Synth::new(0.8);
            s.arpeggio(&[392.0, 523.0, 659.0, 784.0, 1046.0], 0.13, 0.36, Triangle);
            s.finish()
        }
        Sfx::Build => {
            let mut s = Synth::new(0.3);
            s.noise(0.5, 18.0, 0.4, 11);
            s.arpeggio(&[330.0, 494.0], 0.1, 0.3, Square);
            s.finish()
        }
        Sfx::Place => {
            let mut s = Synth::new(0.12);
            s.tone(260.0, 180.0, 0.4, 28.0, Square);
            s.finish()
        }
        Sfx::Recruit => {
            let mut s = Synth::new(0.5);
            s.arpeggio(&[262.0, 392.0, 523.0], 0.14, 0.32, Triangle);
            s.finish()
        }
        Sfx::Order => {
            let mut s = Synth::new(0.16);
            s.arpeggio(&[660.0, 880.0], 0.07, 0.3, Square);
            s.finish()
        }
        Sfx::Capture => {
            let mut s = Synth::new(0.7);
            s.arpeggio(&[440.0, 554.0, 659.0, 880.0], 0.14, 0.34, Sine);
            s.finish()
        }
        Sfx::Lost => {
            let mut s = Synth::new(0.7);
            s.arpeggio(&[660.0, 554.0, 440.0, 330.0], 0.14, 0.34, Saw);
            s.finish()
        }
        Sfx::Surge => {
            let mut s = Synth::new(0.9);
            s.tone(120.0, 900.0, 0.45, 2.0, Saw);
            s.noise(0.35, 3.5, 0.85, 12);
            s.finish()
        }
        Sfx::Tick => {
            let mut s = Synth::new(0.05);
            s.tone(1500.0, 1500.0, 0.3, 60.0, Square);
            s.finish()
        }
        Sfx::Deny => {
            let mut s = Synth::new(0.2);
            s.tone(200.0, 130.0, 0.45, 16.0, Square);
            s.finish()
        }
        Sfx::WaveStart => {
            let mut s = Synth::new(1.0);
            s.tone(90.0, 70.0, 0.5, 2.4, Sine);
            s.arpeggio(&[196.0, 196.0, 262.0], 0.2, 0.34, Saw);
            s.finish()
        }
    }
}

fn build_bank(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let clips = Sfx::ALL
        .iter()
        .map(|sfx| {
            sources.add(AudioSource {
                bytes: render(*sfx).into(),
            })
        })
        .collect();
    commands.insert_resource(SoundBank { clips });
}

fn play_sfx(
    mut commands: Commands,
    bank: Option<Res<SoundBank>>,
    mut events: MessageReader<SfxEvent>,
) {
    let Some(bank) = bank else {
        events.clear();
        return;
    };

    // Collapse duplicates within a frame. Forty darts firing on the same tick
    // should be one louder dart, not forty overlapping copies that clip.
    let mut seen: Vec<(Sfx, f32, u32)> = Vec::new();
    for ev in events.read() {
        if let Some(entry) = seen.iter_mut().find(|(s, _, _)| *s == ev.sound) {
            entry.1 = entry.1.max(ev.volume);
            entry.2 += 1;
        } else {
            seen.push((ev.sound, ev.volume, 1));
        }
    }

    for (sound, volume, count) in seen {
        // Log-ish growth so a big stack is louder but never overwhelming.
        let stack = 1.0 + (count as f32).ln().max(0.0) * 0.28;
        let gain = (sound.gain() * volume * stack).min(1.0);
        commands.spawn((
            AudioPlayer(bank.get(sound)),
            PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::Linear(gain),
                ..default()
            },
            OneShot(2.0),
        ));
    }
}

/// `PlaybackMode::Despawn` normally handles this, but a sink that never starts
/// (tab backgrounded before the audio context resumed) would otherwise leak.
fn reap_one_shots(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut OneShot)>,
) {
    let dt = time.delta_secs();
    for (entity, mut shot) in &mut q {
        shot.0 -= dt;
        if shot.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
