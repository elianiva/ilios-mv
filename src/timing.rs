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
pub const MITEMITAKATTA: f32 = 139.76;
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

/// Camera move over a window that runs past the landing. It starts from rest, runs about 6%
/// past the target and eases back onto it, so it never stops dead.
pub fn whip(x: f32) -> f32 {
    let x = clamp01(x);
    let q = x * x * x * (x * (6.0 * x - 15.0) + 10.0);
    q + 1.8 * q * q * (1.0 - q)
}

/// After the camera lands on a line it keeps gliding into it for this long.
pub const SETTLE: f32 = 1.3;

/// 届くはずのない願いを君は / 僕は: time stops (camera, circles, dust, drift) until the fill.
/// The second is the first moved by the distance between the choruses.
pub const TIME_STOP: (f32, f32) = (76.0, 80.55);
pub const TIME_STOPS: [(f32, f32); 2] = [TIME_STOP, (TIME_STOP.0 + CHORUS_2 - CHORUS_1, TIME_STOP.1 + CHORUS_2 - CHORUS_1)];

pub fn time_stopped(t: f32, margin: f32) -> bool {
    TIME_STOPS.iter().any(|(a, b)| t >= a - margin && t < b + margin)
}

/// The clock the world runs on: it halts while time stops, then lurches back.
pub fn world_clock(t: f32) -> f32 {
    let mut c = t;
    for (a, b) in TIME_STOPS {
        if t >= b {
            c += -(b - a) + 6.0 * expo_out((t - b) / 0.9);
        } else if t >= a {
            c -= t - a;
        }
    }
    c
}

// Cues labeled by ear against the audio (tools/cues), in seconds.
/// The picked guitar notes of the ending.
pub const GUITAR: &[f32] = &[
    254.295, 254.465, 254.595, 254.754, 254.863, 254.992, 255.193, 255.662, 255.979, 256.101, 256.401, 256.700, 256.989, 257.308, 257.448, 257.757,
    257.897, 258.091, 258.506, 258.795, 258.965, 259.114, 259.295, 259.437, 259.573, 259.693, 260.002, 260.265, 260.601, 261.479, 261.788, 262.078,
    262.397, 263.515, 263.903, 264.183, 265.332, 265.689,
    // The three hits after the last labelled pick, one beat apart or two, from the onsets of
    // the 2-8 kHz band (each as strong as the labelled picks round it).
    266.000, 267.800, 268.690,
];
/// The pitch of each guitar pick (MIDI), estimated from the spectrum that appears after each
/// onset. Three estimators (free-pitch harmonic sum, a vote over window sizes, a semitone
/// template limited to the gap before the next pick) agree on 24 picks; the other 14 take the
/// majority and are a best guess: picks 2, 4–8, 17, 18, 21, 27, 29, 30, 33, 36. The mix is
/// about 7 cents sharp, and the line fits B♭ major / G minor. The last three are a rough guess.
pub const GUITAR_NOTES: &[u8] = &[
    54, 82, 60, 82, 82, 66, 58, 81, 58, 70, 72, 77, 70, 79, 79, 81, 77, 77, 69, 69, 69, 67, 65, 74, 60, 75, 60, 75, 58, 60,
    74, 65, 74, 70, 67, 72, 67, 81, 43, 50, 65,
];
const _: () = assert!(GUITAR_NOTES.len() == GUITAR.len());

/// Where a note sits on a treble staff: diatonic steps above E4, the bottom line. Black
/// keys are spelled as flats (the key is B♭ major).
pub fn staff_step(midi: u8) -> f32 {
    const LETTER: [i32; 12] = [0, 1, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6];
    let m = midi as i32;
    ((m / 12 - 1) * 7 + LETTER[(m % 12) as usize] - 30) as f32
}
/// A single low note that lands like something dropped into water.
pub const DROPS: &[f32] = &[2.298, 6.788, 9.502, 98.599, 102.191, 105.793, 109.395, 111.201, 114.782, 118.404, 272.464, 275.996, 279.209, 282.501];
pub const KEYS: &[f32] = &[
    1.500, 2.298, 2.583, 2.886, 3.187, 4.044, 4.963, 5.900, 6.788, 8.427, 9.502, 9.805, 10.119, 10.457, 11.248, 12.196, 13.044, 13.991, 14.440, 14.875,
    15.718, 16.695, 17.593, 18.491, 19.389, 20.247, 21.195, 22.093, 22.991, 23.899, 24.787, 25.695, 26.593, 27.501, 28.399, 29.297, 92.250, 93.150,
    94.129, 94.997, 269.591, 270.432, 270.728, 271.037, 271.350, 272.265, 273.165, 274.061, 274.998, 276.791, 277.757, 278.021, 278.305, 278.605,
    279.520, 280.416, 281.318, 282.121, 282.591, 283.068,
];
/// The big synth swell.
pub const WHEEZE: &[f32] = &[
    15.718, 17.593, 19.389, 20.247, 22.991, 24.787, 26.593, 27.501, 58.091, 62.591, 64.402, 66.193, 67.008, 69.795, 71.591, 73.386, 74.294, 75.202,
    80.590, 81.448, 84.182, 85.988, 87.794, 88.702, 89.600, 121.976, 127.394, 145.393, 147.199, 148.995, 149.843, 152.557, 154.382, 156.089, 157.096,
    157.994, 163.392, 164.250, 166.994, 168.790, 170.596, 171.494,
];
pub const SNARE: &[f32] = &[
    31.093, 32.879, 34.694, 36.440, 38.296, 40.092, 41.848, 43.694, 126.496, 130.088, 135.495, 137.281, 139.097, 140.893, 142.699, 142.998, 143.298,
    143.597, 144.046, 144.495, 175.070, 176.891, 178.697, 180.493, 182.289, 184.055, 185.851, 189.473, 191.289, 193.074, 194.890, 196.547, 198.472,
    200.298, 202.084,
];
pub const CHORD: &[f32] = &[44.592, 45.480, 46.388, 47.286, 48.164, 49.062, 50.000, 50.888, 51.796, 52.694, 53.591, 54.489, 55.347, 56.295, 57.193];
pub const VOCAL_ECHO: &[f32] = &[175.083, 176.891, 178.697, 185.851];

/// Sum of decaying pulses, one per cue at or before `t`.
pub fn cue_pulse(cues: &[f32], t: f32, tau: f32) -> f32 {
    cues.iter().filter(|c| t >= **c && t < **c + tau * 6.0).map(|c| (-(t - c) / tau).exp()).sum()
}

/// Like `cue_pulse`, with a short rise so a swell does not snap on.
pub fn cue_swell(cues: &[f32], t: f32, rise: f32, tau: f32) -> f32 {
    cues.iter().filter(|c| t >= **c && t < **c + tau * 6.0).map(|c| clamp01((t - c) / rise) * (-(t - c) / tau).exp()).sum()
}
