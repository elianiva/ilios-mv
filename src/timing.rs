//! Musical clock of イリオス, measured from `assets/audio.flac`.
//!
//! Beat period and phase come from a spectral-flux comb search over 16–270 s; the period
//! is stable (0.900 s) across the whole song. The first downbeat is the full-band entry
//! at 15.771 s; the chorus (61.68 s) and the 3:30 hit both land on downbeats of this grid.

pub const BEAT: f32 = 0.900;
pub const BAR: f32 = BEAT * 3.0; // 3/4
/// The band's first full downbeat (measured).
pub const DOWNBEAT_0: f32 = 15.771;

/// Downbeat `n` (negative for the intro before the band).
pub fn downbeat(n: i32) -> f32 {
    DOWNBEAT_0 + n as f32 * BAR
}

// Section boundaries (seconds of the song).
pub const BAND_IN: f32 = DOWNBEAT_0;
pub const VERSE_1: f32 = 29.30;
pub const PRE_1: f32 = 44.00;
/// 青い世界 is sung here: the first blue in the video.
pub const BLUE_WORLD: f32 = 50.61;
pub const SMILE: f32 = 58.10;
/// 何度も, chorus 1, on a downbeat.
pub const CHORUS_1: f32 = 61.68;
pub const KURUSHIKU: f32 = 70.39;
pub const SABISHIKU: f32 = 72.12;
pub const MUKOU_E: f32 = 90.51;
pub const INTERLUDE: f32 = 94.0;
// Second half (measured from the waveform; see analysis.md).
pub const VERSE_2: f32 = 112.39;
/// The band slams back in under 鉛のような (one of the strongest onsets, 4.9).
pub const BAND_BACK: f32 = 126.48;
pub const MITEMITAKATTA: f32 = 140.12;
pub const CHORUS_2: f32 = 144.51;
pub const SABISHIKU_2: f32 = 154.94;
pub const BRIDGE: f32 = 172.95;
/// Second half of the bridge: 「月」 / アメジスト.
pub const BRIDGE_MOON: f32 = 187.41;
/// The floor drops out under 「願いが叶う星なんだ」 (-9 dB → -22 dB).
pub const BREAKDOWN: f32 = 201.8;
/// The band returns under と そう言って君は笑う.
pub const BAND_RETURN: f32 = 210.18;
/// The short dip under 願いは, then the strongest hit of the song (5.5).
pub const FINAL: f32 = 219.18;
pub const OITE: f32 = 239.9;
pub const LAST_LINE: f32 = 246.2;
/// The band drops to the piano outro.
pub const OUTRO: f32 = 269.0;
/// Single chimes in the outro.
pub const CHIMES: [f32; 4] = [272.45, 275.98, 279.19, 282.48];
pub const END: f32 = 286.0;

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// 0 → 1 between `a` and `b`, eased in and out.
pub fn ease_between(t: f32, a: f32, b: f32) -> f32 {
    let x = clamp01((t - a) / (b - a));
    x * x * (3.0 - 2.0 * x)
}

pub fn ease_out(x: f32) -> f32 {
    let x = clamp01(x);
    1.0 - (1.0 - x).powi(3)
}

pub fn ease_in(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * x
}

/// The few real edges of the song. From 15.8 s the mix is a dense, compressed wall
/// (flat-topped waveform); what moves are the gaps: the band's entry, the pre-chorus
/// stop-hits (each right after a silence), the chorus downbeat after the 59.5–61.7 s
/// breath, and one fill at 80.3 s. Strength = how hard the picture reacts (0..1).
pub const IMPACTS: &[(f32, f32)] = &[
    (15.79, 0.7),
    (47.28, 0.3), (49.09, 0.3), (49.98, 0.4), (50.88, 0.75),
    (61.69, 0.55),
    (80.59, 0.25),
    (126.48, 0.6),
    (CHORUS_2, 0.35),
    (210.18, 0.35),
    (FINAL, 0.85),
];

/// The band drops out under 向こうへと (RMS -10 dB → -19 dB between 90 and 92 s).
pub const BAND_DROP: f32 = 91.4;

/// Sum of decaying pulses from the impacts: 1.0 right on a full-strength hit.
pub fn impact(t: f32, decay: f32) -> f32 {
    IMPACTS
        .iter()
        .filter(|(ti, _)| t >= *ti && t < ti + decay * 6.0)
        .map(|(ti, s)| s * (-(t - ti) / decay).exp())
        .sum()
}

/// Intro piano notes (spectral flux 300–4000 Hz, peaks above 0.35 of the 99th percentile),
/// in three phrases that each open on a chime (2.28, 6.77, 9.48). Each note lights a star.
pub const PIANO: [&[f32]; 3] = [
    &[1.38, 1.69, 1.85, 2.28, 2.60, 2.89, 3.20, 3.55, 3.80, 4.08, 4.32, 4.54, 4.99, 5.42, 5.80, 6.37],
    &[6.77, 6.97, 7.22, 7.57, 8.02, 8.35, 8.62, 8.79, 9.13],
    &[9.48, 9.80, 10.09, 10.41, 10.75, 10.99, 11.18, 12.19, 12.49, 12.82, 12.99, 13.53, 13.77, 13.98, 14.42],
];
/// Where each intro constellation hangs (world space).
pub const CONSTELLATIONS: [[f32; 3]; 3] = [[-6.0, 4.0, -20.0], [10.0, -3.0, -28.0], [-2.0, 9.0, -40.0]];
/// 笑顔で言う: the camera falls into the text and through a tunnel (59.9 → 61.69).
pub const WORMHOLE: f32 = 59.95;

// ---------------------------------------------------------------- easing
// Exponential-out arrivals (90% of the move in the first ~30% of the time, then a long soft
// tail), expo-in exits, a light spring for the heavy words, and an asymmetric camera whip.

/// 0 → 1, fast start, long tail. 90% done at x ≈ 0.33.
pub fn expo_out(x: f32) -> f32 {
    let x = clamp01(x);
    (1.0 - 2.0_f32.powf(-10.0 * x)) / (1.0 - 2.0_f32.powf(-10.0))
}

/// 0 → 1, slow start, accelerating to the end (exits).
pub fn expo_in(x: f32) -> f32 {
    let x = clamp01(x);
    (2.0_f32.powf(10.0 * x) - 1.0) / (2.0_f32.powf(10.0) - 1.0)
}

/// Damped spring from 0 to 1 over seconds `s` (about 10% overshoot, settled by ~0.6 s).
pub fn spring(s: f32) -> f32 {
    if s <= 0.0 {
        return 0.0;
    }
    let (zeta, omega) = (0.6_f32, 13.0_f32);
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    1.0 - (-zeta * omega * s).exp() * ((wd * s).cos() + zeta * omega / wd * (wd * s).sin())
}

/// Camera whip over a window that runs past the landing: a quick acceleration, then one
/// long exponential glide (no dead stop). Velocity is continuous at the join.
pub fn whip(x: f32) -> f32 {
    // Starts from rest, is quickest a third of the way in and then glides for a long time.
    let x = clamp01(x);
    let y = 1.0 - (1.0 - x) * (1.0 - x);
    y * y * y * (y * (6.0 * y - 15.0) + 10.0)
}

/// After the camera lands on a line it keeps gliding into it for this long.
pub const SETTLE: f32 = 1.3;

/// 届くはずのない願いを君は: time stops (camera, circles, dust, drift) until the fill.
pub const TIME_STOP: (f32, f32) = (76.0, 80.55);

/// The clock the world runs on: it halts while time stops, then lurches back.
pub fn world_clock(t: f32) -> f32 {
    let (a, b) = TIME_STOP;
    if t < a { t } else if t < b { a } else { t - (b - a) + 6.0 * expo_out((t - b) / 0.9) }
}
