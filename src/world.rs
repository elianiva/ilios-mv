//! The 2.5D world: no solid objects, only big flat Mincho glyphs and hairline graphics
//! laid out in 3D space. Every line hangs at its own "station"; the camera follows the word
//! being sung, whips to the next station before the next phrase, and crashes in on the
//! hits. In the chorus the stations sit at the hours of the dial and the camera is the
//! clock's missing hand.
use crate::graphics::{Audio, Graphics};
use crate::lyrics::{Line, Word};
use crate::timing::*;
use fframes::three_d::*;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

const FONT_BYTES: &[u8] = include_bytes!("../media/ShipporiMincho-ExtraBold.ttf");

/// The lyric font. `ILIOS_FONT=<path to a .ttf>` swaps it for one run, to try other faces.
pub fn lyric_font() -> &'static [u8] {
    static FONT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        std::env::var_os("ILIOS_FONT").map_or_else(|| FONT_BYTES.to_vec(), |path| std::fs::read(&path).expect("ILIOS_FONT"))
    })
}
pub(crate) const LABEL_FONT: &[u8] = include_bytes!("../media/ShipporiMincho-Medium.ttf");
/// Box plates are drawn in this colour behind boxed glyphs; boxed glyphs turn dark.
const INK: u32 = 0x150e1a;

/// Integral-style boxed letters: each character on its own tilted square plate.
const BOXED: &[(usize, &str)] = &[
    (1, "君"), (1, "音"), (2, "起きる"), (3, "天井"), (5, "笑顔"), (7, "鉄"), (8, "苦"),
    (10, "独り"), (10, "きり"), (11, "向かう"),
    (16, "鉛"), (16, "足"),
    (20, "鉄"), (21, "苦"), (22, "僕"), (23, "独り"), (23, "きり"), (24, "迎える"),
    (26, "エメラルド"), (29, "アメジスト"), (4, "青い"), (4, "世界"),
    (34, "\"僕\""), (35, "独り"), (35, "きり"), (37, "僕"),
];

/// Lines whose glyphs fly in from scattered depth and assemble (magnetic-style).
const ASSEMBLE: &[usize] = &[];
/// Lines that leave receding copies of themselves behind.
const ECHO: &[usize] = &[6, 7, 19, 20];
/// YOASOBI-style clusters (何度も繰り返し響き渡る / 鉄が奏でるメロディア): each word flips in
/// out of the depth and lands big in the middle; the words already there slide aside, shrink
/// and dim; at the end of the line the whole cluster spins and collapses.
const CLUSTER: &[usize] = &[];

const FOG: u32 = 0x140d19;
const TEXT: u32 = 0xe4d2d7;
const RED_HOT: u32 = 0xff7188;
const BLUE_HOT: u32 = 0x8fb0ff;
const LILAC: u32 = 0x9d93d6;
const EMERALD: u32 = 0x3fe0a6;
const AMETHYST: u32 = 0xc39bff;

// ---------------------------------------------------------------- stations
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub pos: Vec3,
    pub rot: Quat,
}

impl Station {
    fn at(pos: [f32; 3], yaw: f32, pitch: f32, roll: f32) -> Self {
        Self {
            pos: Vec3::from(pos),
            rot: Quat::from_rotation_y(deg(yaw)) * Quat::from_rotation_x(deg(pitch)) * Quat::from_rotation_z(deg(roll)),
        }
    }
    fn world(&self, local: Vec3) -> Vec3 {
        self.pos + self.rot * local
    }
}

/// The choruses: the stations are the hours of the dial. Chorus 1 goes clockwise from
/// 12 (君 heads out); chorus 2 goes counter-clockwise (僕 waits / receives).
/// Where 君 goes, and where one star hangs at the end.
const KIMI_DIR: Vec3 = Vec3::new(0.05, 0.10, -1.0);
/// Depth between the words of a punch line.
const PUNCH_D: f32 = 7.0;
/// Held notes: (line, letter, its word starts after, from, to, how many widths it grows).
/// The letter stretches for as long as the voice holds it and pushes its neighbours along.
const HOLDS: &[(usize, char, f32, f32, f32, f32)] = &[
    (9, 'く', 72.9, 73.2, 75.4, 3.0),
    (13, 'と', 91.0, 91.5, 93.7, 2.4),
    (19, 'て', 138.2, 138.3, 139.7, 1.6),
    (19, 'た', 139.9, 140.05, 141.7, 1.6),
    (19, 'か', 141.78, 141.9, 143.2, 2.6),
    (22, 'く', 155.7, 156.0, 158.0, 3.0),
    (37, 'う', 231.5, 232.1, 234.7, 3.2),
    (38, 'て', 240.5, 241.0, 245.4, 3.0),
    (39, 'て', 247.5, 248.2, 254.5, 4.0),
];
/// How far in front of the first 僕を置いて the second one hangs (metres, toward the camera).
const LAST_GAP: f32 = 31.0;
const HAND_R: f32 = 9.6;
const HAND_Z: f32 = 2.6;
fn hour_station(i: usize, clockwise: bool) -> Station {
    let step = if clockwise { -60.0 } else { 60.0 };
    let a = deg(90.0 + step * i as f32);
    Station { pos: Vec3::new(HAND_R * a.cos(), HAND_R * a.sin(), HAND_Z), rot: Quat::from_rotation_z(a - PI / 2.0) }
}

/// Bridge: the gem diagram floats deep down the corridor.
pub const GEM: Vec3 = Vec3::new(0.0, 0.5, -46.0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Flow {
    /// 縦書き, columns right to left.
    Vertical,
    /// Rows top to bottom.
    Horizontal,
    /// Rows bottom to top (高く高く).
    Stack,
    /// One word at a time, each deeper than the last: the camera flies through them.
    Punch,
    /// The same toward us: the camera retreats and each word arrives from behind it.
    PunchBack,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Entrance {
    /// Comes out of the depth with a small overshoot.
    Drop,
    /// Hits from far bigger, fast.
    Slam,
    /// Floats up from below.
    Rise,
    /// Sinks down from above, heavy (鉛).
    Fall,
    /// Comes down out of the light, stretched tall, and cools from white to its colour.
    Descend,
}

#[derive(Clone, Copy, Debug)]
struct Shot {
    /// Camera distance from the sung word, along the station's facing.
    dist: f32,
    fov: f32,
    /// Where the text sits on screen, in meters at the text (the camera shifts the other way).
    ox: f32,
    oy: f32,
    /// Seconds of the whip from the previous station (0 = hard cut).
    whip: f32,
}

#[derive(Clone, Copy, Debug)]
struct LineSpec {
    station: Station,
    flow: Flow,
    size: f32,
    align: f32,
    entrance: Entrance,
    shot: Shot,
    /// After the last word the line drifts: +1 up, -1 down, 0 stays.
    exit: f32,
    /// Moves to and from this station sweep around the dial instead of cutting across.
    arc: bool,
}

/// Per-word overrides: (line, word, scale, colour, slam).
const WORD_STYLES: &[(usize, &str, f32, Option<u32>, bool)] = &[
    (0, "隅", 1.4, None, false),
    (1, "君", 1.5, None, false),
    (1, "音", 1.5, None, false),
    (2, "起きる", 1.3, None, true),
    (3, "高", 1.35, None, false),
    (3, "く", 1.35, None, false),
    (3, "天井", 1.2, None, false),
    (3, "向こう", 1.3, None, false),
    (4, "青い", 1.5, Some(BLUE_HOT), true),
    (4, "世界", 1.5, Some(BLUE_HOT), true),
    (5, "笑顔", 1.3, None, false),
    (6, "繰り返し", 1.2, None, false),
    (7, "鉄", 1.5, None, true),
    (7, "メロディア", 1.1, None, false),
    (8, "苦", 1.5, Some(RED_HOT), true),
    (8, "しく", 1.5, Some(RED_HOT), false),
    (8, "寂", 1.0, Some(RED_HOT), false),
    (9, "願い", 1.3, None, false),
    (10, "独り", 1.6, None, false),
    (10, "きり", 1.6, None, false),
    (11, "向かう", 1.4, None, false),
    (12, "向かう", 1.3, None, false),
    // Verse 2 / pre-chorus 2.
    (13, "二人", 1.4, None, false),
    (13, "希望", 1.2, None, false),
    (15, "違う", 1.3, None, false),
    (15, "夢", 1.5, None, false),
    (16, "鉛", 1.7, None, true),
    (16, "足", 1.5, None, false),
    (16, "睨", 1.3, Some(RED_HOT), true),
    (17, "同じ", 1.3, None, false),
    (17, "夢", 1.5, None, false),
    // Chorus 2.
    (19, "繰り返し", 1.2, None, false),
    (20, "鉄", 1.5, None, true),
    (20, "メロディア", 1.1, None, false),
    (21, "苦", 1.5, Some(RED_HOT), true),
    (21, "しく", 1.5, Some(RED_HOT), false),
    (21, "寂", 1.0, Some(RED_HOT), false),
    (22, "僕", 1.5, None, false),
    (23, "独り", 1.6, None, false),
    (23, "きり", 1.6, None, false),
    (24, "迎える", 1.4, None, false),
    // Bridge: the only colour in the video. 「星」 and 「月」 stay small and cold.
    (25, "星", 0.7, Some(LILAC), false),
    (26, "エメラルド", 1.3, Some(EMERALD), false),
    (27, "目", 1.6, Some(EMERALD), false),
    (27, "好き", 1.3, None, false),
    (28, "月", 0.7, Some(LILAC), false),
    (29, "アメジスト", 1.3, Some(AMETHYST), false),
    (30, "声", 1.6, Some(AMETHYST), false),
    (30, "好き", 1.3, None, false),
    // Final chorus.
    (34, "\"僕\"", 2.0, None, true),
    (35, "独り", 1.5, None, false),
    (35, "きり", 1.5, None, false),
    (36, "行", 1.5, Some(BLUE_HOT), false),
    (37, "大切", 1.3, None, false),
    (37, "僕", 1.5, None, false),
    (37, "置", 1.3, Some(RED_HOT), false),
    (38, "置", 1.2, Some(RED_HOT), false),
];

/// Extra row breaks after these words (line, word).
const BREAKS: &[(usize, &str)] = &[
    (0, "ない"), (2, "に"), (3, "く"), (4, "いる"), (6, "も"), (6, "繰り返し"), (7, "が"), (7, "奏でる"), (9, "ない"),
    (10, "ただ"), (11, "て"),
    (13, "語らう"), (15, "を"), (16, "な"), (16, "んで"), (17, "夢"),
    (19, "も"), (19, "繰り返し"), (20, "が"), (20, "奏でる"), (22, "ない"), (23, "ただ"), (24, "て"),
    (25, "話す"), (27, "が"), (28, "話す"), (30, "が"),
    (31, "叶う"), (32, "て"),
    (34, "きっと"), (34, "\"僕\""), (35, "ただ"), (37, "な"),
];

/// One flight. Every line hangs ahead of the last on the same course (further along -z), so
/// the camera reaches the next line by flying on through the one it was reading.
fn specs() -> Vec<LineSpec> {
    let shot = |dist, fov, whip| Shot { dist, fov, ox: 0.0, oy: 0.0, whip };
    let line = |pos: [f32; 3], yaw: f32, pitch: f32, roll: f32, flow, size, entrance, shot| LineSpec {
        station: Station::at(pos, yaw, pitch, roll),
        flow,
        size,
        align: if flow == Flow::Vertical { 0.0 } else { 0.5 },
        entrance,
        shot,
        exit: 0.0,
        arc: false,
    };
    let (h, v, st, pu) = (Flow::Horizontal, Flow::Vertical, Flow::Stack, Flow::Punch);
    let (drop, slam, rise) = (Entrance::Drop, Entrance::Slam, Entrance::Rise);
    let mut out = vec![
        // 0-2 Verse 1.
        line([-1.5, 3.4, -6.0], 14.0, 0.0, 0.0, v, 0.78, drop, shot(5.4, 42.0, 0.0)),
        line([2.5, 1.5, -20.0], -12.0, 0.0, 0.0, v, 0.8, drop, shot(5.6, 42.0, 0.6)),
        line([-1.5, 0.0, -34.0], 0.0, 0.0, 8.0, h, 0.95, drop, shot(6.2, 44.0, 0.6)),
        // 3-5 Pre-chorus 1: climbing; 青い世界 is above us.
        line([1.2, -0.5, -48.0], -8.0, 0.0, 0.0, st, 0.85, rise, shot(5.6, 46.0, 0.6)),
        line([0.0, 5.0, -64.0], 0.0, 28.0, 0.0, h, 0.95, Entrance::Descend, shot(6.0, 50.0, 0.6)),
        line([0.0, 2.0, -80.0], 0.0, 0.0, 0.0, h, 0.95, drop, shot(5.0, 40.0, 0.8)),
        // 6-12 Chorus 1: through the words, phrase by phrase.
        line([0.0, 2.0, -98.0], 0.0, 0.0, 0.0, Flow::Punch, 1.35, drop, shot(6.6, 50.0, 0.0)),
        line([1.5, 2.5, -120.0], 0.0, 0.0, 0.0, Flow::Punch, 1.35, drop, shot(6.6, 50.0, 0.7)),
        line([-1.0, 1.5, -144.0], 0.0, 0.0, -6.0, h, 1.1, slam, shot(5.6, 48.0, 0.7)),
        line([0.5, 2.0, -158.0], 0.0, 0.0, 0.0, h, 0.82, drop, shot(5.6, 48.0, 1.0)),
        line([-1.5, 2.5, -172.0], 0.0, 0.0, 5.0, h, 0.85, drop, shot(5.6, 48.0, 0.95)),
        line([0.0, 2.0, -186.0], 0.0, 0.0, 0.0, pu, 0.9, drop, shot(5.6, 48.0, 0.7)),
        // 向こうへと向かうんだ: far ahead on the line of flight, and it only gets further away.
        line([0.5, 2.0, -244.0], 0.0, 0.0, 0.0, h, 3.0, drop, shot(5.6, 48.0, 0.0)),
        // 13-15 Verse 2: phrase after phrase into the depth, each line bending off from the last.
        line([0.0, 10.0, -272.0], 0.0, 0.0, 0.0, pu, 0.8, drop, shot(6.0, 44.0, 1.6)),
        line([1.0, 9.6, -300.0], 22.0, 0.0, 0.0, pu, 0.8, drop, shot(6.0, 44.0, 1.1)),
        line([-5.8, 10.0, -321.0], -20.0, 0.0, 0.0, pu, 0.8, drop, shot(6.0, 44.0, 1.1)),
        // 16-18 Pre-chorus 2: the eyes drop to the town. Heavy.
        line([0.0, -4.0, -362.0], 0.0, -55.0, 0.0, h, 0.9, drop, shot(6.4, 46.0, 0.5)),
        line([-2.0, -5.0, -374.0], 0.0, -55.0, 6.0, h, 0.78, drop, shot(5.8, 44.0, 0.8)),
        line([1.0, -6.0, -386.0], 0.0, -55.0, 0.0, h, 1.0, drop, shot(6.4, 44.0, 0.8)),
        // 19-24 Chorus 2.
        line([0.0, 2.0, -398.0], 0.0, 0.0, 0.0, Flow::Punch, 1.35, drop, shot(6.6, 50.0, 0.0)),
        line([-1.5, 2.5, -420.0], 0.0, 0.0, 0.0, Flow::Punch, 1.35, drop, shot(6.6, 50.0, 0.7)),
        line([1.0, 1.5, -444.0], 0.0, 0.0, 6.0, h, 1.1, slam, shot(5.6, 48.0, 0.7)),
        line([-0.5, 2.0, -458.0], 0.0, 0.0, 0.0, h, 0.82, drop, shot(5.6, 48.0, 0.9)),
        line([1.5, 2.5, -472.0], 0.0, 0.0, -5.0, h, 0.85, drop, shot(5.6, 48.0, 0.8)),
        line([0.0, 2.0, -486.0], 0.0, 0.0, 0.0, h, 0.9, drop, shot(5.6, 48.0, 0.7)),
        // 25-30 Bridge.
        line([2.4, 3.0, -500.0], -8.0, 0.0, 0.0, h, 0.6, rise, shot(6.4, 40.0, 1.4)),
        line([-2.0, 1.2, -512.0], 10.0, 0.0, 0.0, h, 1.05, rise, shot(6.0, 44.0, 1.2)),
        line([1.0, 2.4, -524.0], -4.0, 0.0, 0.0, h, 0.8, rise, shot(6.0, 42.0, 1.2)),
        line([-2.4, 3.0, -536.0], 8.0, 0.0, 0.0, h, 0.6, rise, shot(6.4, 40.0, 1.6)),
        line([2.0, 1.2, -548.0], -10.0, 0.0, 0.0, h, 1.05, rise, shot(6.0, 44.0, 1.2)),
        line([-1.0, 2.4, -560.0], 4.0, 0.0, 0.0, h, 0.8, rise, shot(6.0, 42.0, 1.2)),
        // 31-33 Breakdown: a run at the wish star.
        line([0.0, 1.0, -584.0], 0.0, 0.0, 0.0, pu, 0.8, drop, shot(6.4, 38.0, 1.8)),
        line([0.6, 1.2, -614.0], -8.0, 0.0, 0.0, pu, 0.8, drop, shot(6.2, 40.0, 1.0)),
        line([4.2, 1.6, -645.0], -8.0, 0.0, 0.0, h, 1.2, slam, shot(7.4, 44.0, 0.8)),
        // 34-38 Last chorus: level with the horizon, where the dawn is. Then down, as it closes.
        line([0.0, 3.0, -666.0], 0.0, -6.0, 0.0, h, 0.72, slam, shot(6.8, 50.0, 0.45)),
        line([-3.0, 4.5, -678.0], 10.0, -6.0, 0.0, h, 0.85, drop, shot(6.0, 48.0, 0.8)),
        line([2.0, 6.0, -690.0], -6.0, -4.0, 0.0, h, 1.0, rise, shot(6.6, 50.0, 0.8)),
        line([0.0, 2.0, -702.0], 0.0, -30.0, 0.0, h, 0.8, drop, shot(6.6, 46.0, 1.2)),
        // The second 僕を置いて hangs on the same axis as the first, LAST_GAP nearer to us: the
        // camera never turns back for it, it appears in front of the camera as that keeps leaving.
        line([0.0, 2.0 + 0.5 * LAST_GAP, -702.0 + 0.866 * LAST_GAP], 0.0, -30.0, 0.0, h, 0.5, drop, shot(5.8, 42.0, 1.4)),
    ];
    out[24].exit = -1.0;
    out[36].exit = 1.0;
    out[38].shot.oy = -0.8;
    out
}

// ---------------------------------------------------------------- built lyric lines
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Main,
    /// A receding copy, `n` steps back.
    Echo(u8),
    /// A faint trailing copy `n` steps behind in time (fake per-letter motion blur).
    Ghost(u8),
}

struct GlyphInst {
    kind: Kind,
    plate: Option<NodeId>,
    tilt: f32,
    /// Start offset for assembling glyphs (station space).
    scatter: Vec3,
    node: NodeId,
    line: usize,
    appear: f32,
    word_start: f32,
    word_end: f32,
    base: Vec3,
    color: Rgb,
    plate_color: Rgb,
    slam: bool,
    /// Cluster lines: (word index, direction it is pushed when newer words land).
    cluster: Option<(usize, f32)>,
    /// The word's accent: the letter stays white and glows in this colour.
    accent: Option<u32>,
    /// Advance of the glyph at its size (for stretching held notes).
    width: f32,
    ch: char,
}

struct LineInst {
    /// Cluster lines: when each word lands.
    arrivals: Vec<f32>,
    group: NodeId,
    spec: LineSpec,
    in_t: f32,
    out_t: f32,
    start: f32,
    end: f32,
    /// (sung time, word centre in station space)
    words: Vec<(f32, Vec3)>,
}

// ---------------------------------------------------------------- camera
#[derive(Clone, Copy, Debug)]
pub struct CamPose {
    pub pos: Vec3,
    pub look: Vec3,
    pub fov: f32,
    /// Desired screen-up direction (world); the roll is derived from it.
    pub up: Vec3,
}

impl CamPose {
    pub fn camera(&self) -> Camera {
        let base = Camera::perspective(self.fov).position(self.pos).look_at(self.look);
        let fwd = base.forward();
        let up0 = base.up_vector();
        let want = (self.up - fwd * self.up.dot(fwd)).normalize_or_zero();
        if want == Vec3::ZERO {
            return base;
        }
        let roll = up0.cross(want).dot(fwd).atan2(up0.dot(want));
        base.roll(roll)
    }
}

fn wrap_pi(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn smooth5(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * x * (x * (6.0 * x - 15.0) + 10.0)
}

/// Blend two poses. `arc`: interpolate around the clock centre. `roll`: how far the camera
/// banks in the middle of the move (radians); `turns`: whole barrel rolls on the way.
fn blend(a: &CamPose, b: &CamPose, e: f32, arc: bool, roll: f32, turns: f32) -> CamPose {
    let lerp_pos = |p: Vec3, q: Vec3| -> Vec3 {
        if !arc {
            return p.lerp(q, e);
        }
        let (ap, aq) = (p.y.atan2(p.x), q.y.atan2(q.x));
        let ang = ap + wrap_pi(aq - ap) * e;
        let r = Vec2::new(p.x, p.y).length() * (1.0 - e) + Vec2::new(q.x, q.y).length() * e;
        Vec3::new(r * ang.cos(), r * ang.sin(), p.z + (q.z - p.z) * e)
    };
    // Up vectors: rotate from one to the other (keeps the roll continuous).
    let up = Quat::from_rotation_arc(a.up.normalize(), b.up.normalize());
    let up = Quat::IDENTITY.slerp(up, e) * a.up;
    let mut pos = lerp_pos(a.pos, b.pos);
    let bump = (e * PI).sin();
    // The path bows out sideways instead of running straight: it reads as drifting.
    let travel = b.pos - a.pos;
    if !arc && travel.length() > 0.5 {
        let side = travel.cross(up).normalize_or_zero();
        pos += side * bump * (0.1 * travel.length()).min(3.0) * roll.signum();
    }
    // The head turns by heading and pitch, so even a turn right round stays level.
    let (da, db) = (a.look - a.pos, b.look - b.pos);
    let yp = |d: Vec3| (d.x.atan2(-d.z), (d.y / d.length().max(0.01)).clamp(-1.0, 1.0).asin());
    let ((ya, pa), (yb, pb)) = (yp(da), yp(db));
    let (y, p) = (ya + wrap_pi(yb - ya) * e, pa + (pb - pa) * e);
    let look = pos + Vec3::new(y.sin() * p.cos(), p.sin(), -y.cos() * p.cos()) * (da.length() + (db.length() - da.length()) * e);
    let fwd = (look - pos).normalize_or_zero();
    let up = Quat::from_axis_angle(if fwd == Vec3::ZERO { Vec3::Z } else { fwd }, roll * bump + turns * TAU * smooth5(e)) * up;
    CamPose {
        pos,
        look,
        // Widen in the middle of the move: reads as speed.
        fov: a.fov + (b.fov - a.fov) * e + 7.0 * bump,
        up,
    }
}

/// How the camera banks on the way to line `j`: alternating sides, and a full barrel roll
/// where the song restarts.
fn move_roll(j: usize) -> (f32, f32) {
    let side = if j % 2 == 0 { 1.0 } else { -1.0 };
    (0.14 * side, 0.0)
}

struct Key {
    t: f32,
    pos: Vec3,
    look: Vec3,
    fov: f32,
    /// How we get here from the previous key.
    ease: Ease,
}
#[derive(Clone, Copy, PartialEq)]
enum Ease {
    Cut,
    Linear,
    Smooth,
    /// Accelerates into the key (a pull that lands on a hit).
    In,
}
const fn k(t: f32, pos: [f32; 3], look: [f32; 3], fov: f32, ease: Ease) -> Key {
    Key { t, pos: Vec3::new(pos[0], pos[1], pos[2]), look: Vec3::new(look[0], look[1], look[2]), fov, ease }
}

/// The intro: macro shots of the mechanism cut on the intro's hits, a pull back that lands
/// on the band's first downbeat, then a slow push into the title.
const INTRO: &[Key] = &[
    // Drifting in the dark; the first constellation draws itself to the piano.
    k(0.0, [-3.0, 2.6, -4.0], [-6.0, 4.0, -20.0], 36.0, Ease::Cut),
    k(6.70, [-4.6, 3.4, -9.5], [-6.0, 4.0, -20.0], 31.0, Ease::Linear),
    // 6.77: the second.
    k(6.77, [13.0, -1.2, -16.0], [10.0, -3.0, -28.0], 34.0, Ease::Cut),
    k(9.40, [11.6, -2.2, -18.5], [10.0, -3.0, -28.0], 31.0, Ease::Linear),
    // 9.48: the third.
    k(9.48, [-5.0, 7.2, -27.0], [-2.0, 9.0, -40.0], 36.0, Ease::Cut),
    k(13.9, [-3.4, 8.2, -30.5], [-2.0, 9.0, -40.0], 33.0, Ease::Linear),
    // Pull back past all three; it lands on the band and the title.
    k(15.79, [0.0, 1.0, 31.0], [0.0, 1.0, 0.0], 40.0, Ease::In),
    k(24.70, [0.6, 1.4, 23.0], [0.0, 1.3, 0.0], 38.0, Ease::Smooth),
    k(26.50, [-0.4, 1.6, 20.5], [0.0, 1.5, 0.0], 36.0, Ease::Smooth),
];

fn intro_pose(t: f32) -> CamPose {
    if t < BAND_IN {
        // Flying forward through the sky; the head turns toward whichever constellation is
        // drawing itself, and settles on the title as we arrive.
        let pos = crate::graphics::flight(t);
        let ahead = pos + Vec3::new(0.0, 0.0, -24.0);
        let mut look = ahead;
        for (_, centre, _, times) in crate::graphics::constellation_plan().into_iter().take(6) {
            let (a, b) = (times[0] - 0.8, times[times.len() - 1] + 1.0);
            let w = crate::graphics::window(t, a, b, 2.2);
            look = look.lerp(centre, 0.6 * w);
        }
        let arrive = ease_between(t, BAND_IN - 1.8, BAND_IN);
        look = look.lerp(Vec3::new(0.0, 1.0, 0.0), arrive);
        // Bank into the drift, like something weightless that turns with its whole body.
        let bank = (0.34 * (t * 0.31).cos() + 0.16 * (t * 0.13 + 0.6).sin()) * (1.0 - arrive);
        let up = Quat::from_axis_angle((look - pos).normalize(), bank) * Vec3::Y;
        return CamPose { pos, look, fov: 38.0 + 2.0 * arrive, up };
    }
    let i = INTRO.iter().rposition(|k| k.t <= t).unwrap_or(0);
    let a = &INTRO[i];
    let Some(b) = INTRO.get(i + 1) else {
        return CamPose { pos: a.pos, look: a.look, fov: a.fov, up: Vec3::Y };
    };
    let x = clamp01((t - a.t) / (b.t - a.t));
    let e = match b.ease {
        Ease::Cut => 0.0,
        Ease::Linear => x,
        Ease::Smooth => smooth5(x),
        Ease::In => x * x * x * x,
    };
    CamPose { pos: a.pos.lerp(b.pos, e), look: a.look.lerp(b.look, e), fov: a.fov + (b.fov - a.fov) * e, up: Vec3::Y }
}

// ---------------------------------------------------------------- the world
pub struct World {
    scene: Scene,
    renderer: Renderer,
    text_scene: Scene,
    text_renderer: Renderer,
    graphics: Graphics,
    glyphs: Vec<GlyphInst>,
    lines: Vec<LineInst>,
    /// メロディア's staff: (group, rest position, from, to).
    staffs: Vec<(NodeId, Vec3, f32, f32, f32, Vec<(NodeId, f32, f32)>)>,
    dust: Vec<(Vec3, f32, f32, f32)>,
    /// The second voice's 好きだった, in a scene of its own so the post shader can haze it:
    /// a group that travels with the camera, and its letters as (letter, rest position, when it
    /// is sung).
    angel_scene: Scene,
    angel_renderer: Renderer,
    angel: (NodeId, Vec<(NodeId, Vec3, f32)>),
}

/// The second voice sings 好きだった again, softly, over the move to 「月」: (word, sung at).
const ANGEL: &[(&str, f32)] = &[("好き", 185.571), ("だ", 186.2), ("っ", 186.5), ("た", 186.85)];
const ANGEL_GOLD: u32 = 0xffe2b0;

pub struct Frame3d {
    pub world: fframes::media::ImageData<'static>,
    pub text: fframes::media::ImageData<'static>,
    /// Screen motion of the frame centre during half a frame, in pixels.
    pub pan: Vec2,
    /// Forward motion relative to the distance to the target (radial blur).
    pub zoom: f32,
    /// Roll during half a frame, radians.
    pub spin: f32,
    pub hit: f32,
    pub blue: f32,
    pub ray_color: Vec3,
    pub red_flash: f32,
    pub world_light: f32,
    pub invert: f32,
    pub red_frame: f32,
    pub glitch: f32,
    pub speed: f32,
    /// 0..1: the wormhole twist under 笑顔で言う.
    pub twirl: f32,
    /// Camera basis for the sky in the shader.
    pub cam_right: Vec3,
    pub cam_up: Vec3,
    pub cam_fwd: Vec3,
    pub tan_half_fov: f32,
    pub low: f32,
    pub tv: f32,
    pub crt: f32,
    pub dome: f32,
    pub stop: f32,
    pub flash: f32,
    pub sun_dir: Vec3,
    pub sun_amt: f32,
    /// 0 = totality, 1 = the sun stands clear of the moon.
    pub eclipse: f32,
    pub neb_a: Vec3,
    pub neb_b: Vec3,
    /// Comet tail.xy -> head.xy in uv, and its amount.
    pub comet: [f32; 4],
    pub comet_amt: f32,
    /// 0..1 while the lyrics sit on the chorus dial.
    pub shade: f32,
    pub glow_col: Vec3,
    pub glow_amt: f32,
    pub star_dir: Vec3,
    pub star_amt: f32,
    pub iris: f32,
    pub voice: f32,
    pub beat: f32,
    /// The downbeat toll: (seconds since, amount).
    pub toll: (f32, f32),
    pub swell: f32,
    pub snare: f32,
    /// A dropped note: (screen x, seconds since, amount).
    pub drop: Vec3,
    /// The last two piano keys struck: (screen x, seconds since, amount).
    pub keys: [Vec3; 2],
    /// Ending: the staff of light, and the last two guitar picks on it: (line, seconds since, amount).
    pub staff: f32,
    pub picks: [Vec3; 2],
    /// Vocal echo: (seconds since, amount).
    pub echo: (f32, f32),
    /// Colour of the air on the planet's limb.
    pub atmo: Vec3,
    /// The second voice's 好きだった, drawn hazy behind the lyrics, and how much of it shows.
    pub angel: fframes::media::ImageData<'static>,
    pub angel_amt: f32,
}

/// YOASOBI-style bursts: a red noise frame, then an inverted frame (2 frames each).
const BURSTS: &[f32] = &[];
/// Single inverted frames.
const INVERTS: &[f32] = &[BAND_IN];
const GLITCHES: &[(f32, f32)] = &[(49.09, 0.2), (49.98, 0.25), (CHORUS_1, 0.2), (BAND_BACK, 0.3), (130.69, 0.4), (FINAL, 0.35)];
const SPEED_LINES: &[(f32, f32)] = &[];

fn decay(list: &[(f32, f32)], t: f32, tau: f32) -> f32 {
    list.iter().filter(|(ti, _)| t >= *ti && t < ti + tau * 6.0).map(|(ti, s)| s * (-(t - ti) / tau).exp()).sum()
}

/// An aura round an emphasised letter: three glow copies scaled out round its centre.
fn aura(scene: &mut Scene, glyph: NodeId, geo: &Arc<Geometry>, adv: f32) -> NodeId {
    let group = scene.add_to(glyph, Node::group().at(0.0, 0.0, -0.05));
    let centre = Vec3::new(adv * 0.5, 0.36, 0.0);
    for sc in [1.05_f32, 1.12, 1.22] {
        scene.add_to(
            group,
            Node::mesh(geo.clone(), Material::glow(Rgb::hex(0xffb3d1), 0.0)).with_position(centre * (1.0 - sc)).scaled(sc),
        );
    }
    group
}

/// The aura breathes up while its word is sung and settles to a low glow.
fn pose_echo(ts: &mut Scene, p: NodeId, g: &GlyphInst, t: f32, opacity: f32) {
    // The title's knockout: a dark copy behind the letter (no children).
    if ts.descendants(p).is_empty() {
        if let Some(m) = ts[p].material_mut() {
            m.opacity = opacity * 0.0;
        }
        return;
    }
    let sung = (-(t - g.word_start).max(0.0) / 1.2).exp() * if t >= g.word_start - 0.1 { 1.0 } else { 0.0 };
    let base = (0.22 + 0.45 * sung) * opacity * if g.accent.is_some() { 2.4 } else { 1.0 };
    let kids: Vec<NodeId> = ts.descendants(p);
    for (k, id) in kids.into_iter().enumerate() {
        if let Some(m) = ts[id].material_mut() {
            m.color = g.plate_color.lerp(Rgb::hex(0xffb3d1), if g.accent.is_some() { 0.1 } else { 0.6 }) * (base * [0.35, 0.22, 0.12][k.min(2)]);
        }
    }
}

/// A light that glows: a white-hot core and soft halos of its colour, each wider and fainter.
fn glow(out: &mut Vec<Particle>, pos: Vec3, size: f32, color: Rgb, amt: f32) {
    if amt <= 0.003 {
        return;
    }
    out.push(Particle::new(pos, size * 0.55, Rgb::hex(0xffffff).lerp(color, 0.25) * (5.0 * amt)).opacity(amt));
    out.push(Particle::new(pos, size, color * (3.0 * amt)).opacity(amt));
    for (k, e) in [(2.6, 0.55), (5.0, 0.22), (9.5, 0.08)] {
        out.push(Particle::new(pos, size * k, color * (e * amt)).opacity(e * amt));
    }
}

/// A moving light and the trail it leaves. `path` is where the light was, newest first, at
/// even steps in time. The trail is one unbroken ribbon: sprites are laid closer than their
/// own radius along the path and each carries light in proportion to the length it covers, so
/// a fast light leaves a long thin streak and never a row of beads. The head is a small white
/// point in a soft bloom that falls off smoothly, with no disc edge.
fn light_trail(out: &mut Vec<Particle>, path: &[Vec3], width: f32, color: Rgb, amt: f32) {
    if amt <= 0.003 || path.len() < 2 {
        return;
    }
    let white = Rgb::hex(0xffffff);
    let n = (path.len() - 1) as f32;
    for (k, pair) in path.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let len = (b - a).length();
        let steps = ((len / (width * 0.3)).ceil() as usize).clamp(1, 60);
        for j in 0..steps {
            let f = (j as f32 + 0.5) / steps as f32;
            let u = (k as f32 + f) / n;
            let fade = (1.0 - u).powf(2.2);
            let w = width * (0.18 + 0.82 * (1.0 - u).powf(1.3));
            // Light per sprite: the same light per metre of trail however fast the light moves.
            let per = (len / steps as f32 / (width * 0.3)).min(1.0) * amt * fade;
            let p = a.lerp(b, f);
            out.push(Particle::new(p, w * 0.32, white.lerp(color, 0.35) * (0.9 * per)).opacity(per.min(1.0)));
            out.push(Particle::new(p, w, color * (0.32 * per)).opacity((0.6 * per).min(1.0)));
            out.push(Particle::new(p, w * 3.2, color * (0.035 * per)).opacity((0.12 * per).min(1.0)));
        }
    }
    // The head: many thin layers, each wider and fainter, add up to a smooth falloff.
    let head = path[0];
    out.push(Particle::new(head, width * 0.42, white * (4.0 * amt)).opacity(amt));
    for k in 0..14 {
        let r = width * 0.6 * 1.32_f32.powi(k);
        let e = 0.8 / (1.0 + k as f32 * 0.9).powf(1.6);
        out.push(Particle::new(head, r, white.lerp(color, clamp01(0.3 + k as f32 * 0.12)) * (e * amt)).opacity((e * amt).min(1.0)));
    }
}

fn window_f(t: f32, a: f32, b: f32, fade: f32) -> f32 {
    crate::graphics::window(t, a, b, fade)
}

/// Flight: where the song builds, the picture accelerates, then eases out of it over 1.6 s.
/// (from, to, strength)
const WARPS: &[(f32, f32, f32)] = &[
    (73.2, 75.9, 0.5), (87.6, 90.1, 0.8), (138.3, 144.5, 1.0), (156.0, 158.5, 0.5), (210.2, 219.15, 1.0), (231.9, 235.2, 0.6),
];

fn warp(t: f32) -> f32 {
    WARPS
        .iter()
        .map(|&(a, b, k)| if t < a { 0.0 } else if t < b { k * ((t - a) / (b - a)).powf(1.6) } else { k * (1.0 - smooth5(clamp01((t - b) / 1.6))) })
        .fold(0.0, f32::max)
}

/// How far the flight has carried the dust by `t` (the integral of `warp`, in metres).
fn warp_travel(t: f32) -> f32 {
    WARPS.iter().map(|&(a, b, k)| k * 70.0 * (b - a) * clamp01((t - a) / (b - a)).powf(2.6) / 2.6).sum()
}

/// Which phrase of 鉛のようなこの足を睨んで君を待つ a word belongs to, by when it is sung.
fn lead_row(word_start: f32) -> usize {
    if word_start < 128.9 {
        0
    } else if word_start < 132.4 {
        1
    } else {
        2
    }
}

/// Beat pulse in the choruses: 1 on a downbeat, less on beats 2 and 3, decaying.
fn beat_pulse(t: f32) -> f32 {
    let in_chorus = ((t >= CHORUS_1 && t < 88.0) || (t >= CHORUS_2 && t < 172.0) || (t >= FINAL && t < 246.0)) && !time_stopped(t, 0.0);
    if !in_chorus {
        return 0.0;
    }
    let n = (t - DOWNBEAT_0) / BEAT;
    let k = if (n.floor() as i32).rem_euclid(3) == 0 { 1.0 } else { 0.4 };
    k * (-(n.fract() * BEAT) / 0.16).exp()
}

/// The chorus tolls on every downbeat, like struck iron (鉄が奏でるメロディア): seconds since
/// the last one, and how loud it still rings. Silent while time is stopped.
fn toll(t: f32) -> (f32, f32) {
    let in_chorus = |x: f32| ((x >= CHORUS_1 && x < 88.0) || (x >= CHORUS_2 && x < 172.0) || (x >= FINAL && x < 246.0)) && !time_stopped(x, 0.5);
    let at = downbeat(((t - DOWNBEAT_0) / BAR).floor() as i32);
    if !in_chorus(at) || time_stopped(t, 0.0) {
        return (9.0, 0.0);
    }
    (t - at, (-(t - at) / 0.9).exp())
}

/// 0 → 1 as the camera falls into 笑顔で言う and through the tunnel (accelerating).
fn wormhole(t: f32) -> f32 {
    if t >= CHORUS_1 - 0.04 {
        return 0.0;
    }
    let x = clamp01((t - WORMHOLE) / (CHORUS_1 - 0.04 - WORMHOLE));
    x * x * (0.4 + 0.6 * x)
}

/// Slow start, then lands hard: weight.
fn ease_in_out_heavy(x: f32) -> f32 {
    let x = clamp01(x);
    if x < 0.7 { 0.35 * (x / 0.7).powi(2) } else { 0.35 + 0.65 * ease_out((x - 0.7) / 0.3) }
}

fn hash3(i: u32, s: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E3779B1) ^ s.wrapping_mul(0x85EBCA77);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B3C6D);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

fn title_line() -> Line {
    let words = "イリオス"
        .chars()
        .enumerate()
        .map(|(i, ch)| {
            let start = BAND_IN + i as f32 * BEAT;
            Word { text: ch.to_string(), start, end: start + BEAT, break_after: false }
        })
        .collect();
    Line { start: BAND_IN, words }
}

impl World {
    pub fn new(lines: &[Line], width: u32, height: u32) -> Self {
        let mut scene = Scene::new().fog(Fog::Linear { color: Rgb::hex(FOG), near: 20.0, far: 260.0 });
        let mut text_scene = Scene::new().fog(Fog::Linear { color: Rgb::hex(FOG), near: 14.0, far: 110.0 });

        let graphics = Graphics::build(&mut scene);

        // ---- lyrics
        let mut glyph_cache: HashMap<(bool, char), (Arc<Geometry>, f32)> = HashMap::new();
        let mut glyph_in = |label: bool, ch: char| -> Option<(Arc<Geometry>, f32)> {
            if let Some(g) = glyph_cache.get(&(label, ch)) {
                return Some(g.clone());
            }
            let font = if label { LABEL_FONT } else { lyric_font() };
            let g = Geometry::text_glyphs(font, &ch.to_string(), 1.0, if label { 0.02 } else { 0.1 }, 0.0)
                .ok()?
                .into_iter()
                .next()?;
            let entry = (Arc::new(g.geometry), g.advance);
            glyph_cache.insert((label, ch), entry.clone());
            Some(entry)
        };

        let specs = specs();
        let title = title_line();
        let title_spec = LineSpec {
            station: Station::at([0.0, 1.0, 10.0], 0.0, 0.0, 0.0),
            flow: Flow::Horizontal,
            size: 2.4,
            align: 0.5,
            entrance: Entrance::Slam,
            shot: Shot { dist: 20.0, fov: 40.0, ox: 0.0, oy: 0.0, whip: 0.0 },
            exit: 0.0,
            arc: false,
        };
        let mut all: Vec<(&Line, LineSpec, Option<usize>)> = vec![(&title, title_spec, None)];
        for (i, (l, s)) in lines.iter().zip(specs.iter()).enumerate() {
            all.push((l, *s, Some(i)));
        }

        let mut glyphs = Vec::new();
        let mut line_insts = Vec::new();
        for (li, (line, spec, lrc)) in all.iter().enumerate() {
            let next_start = lrc.and_then(|i| lines.get(i + 1)).map(|l| l.start);
            let first = line.words.first().map_or(line.start, |w| w.start);
            // A line whose back the camera is about to see (the next station faces away)
            // leaves as the camera starts to move, so it never shows mirrored.
            let faces_away = lrc
                .and_then(|i| specs.get(i + 1))
                .is_some_and(|n| n.shot.whip > 0.0 && (n.station.rot * Vec3::Z).dot(spec.station.rot * Vec3::Z) < 0.0);
            let (in_t, out_t) = match (lrc, next_start) {
                (None, _) => (first - 0.2, 29.2),
                // Punch lines clear the way for the next one.
                (Some(6 | 7 | 19 | 20), Some(n)) => (first - 0.4, n - 0.2),
                (_, Some(n)) if faces_away => (first - 0.4, n - 1.2),
                (Some(3), Some(n)) => (first - 0.4, n - 0.25),
                (Some(8), _) => (first - 0.4, 75.7),
                (Some(11), _) => (first - 0.4, 90.1),
                (Some(21), _) => (first - 0.4, 158.25),
                // The first 僕を置いて is gone before the second appears in front of it.
                (Some(37), _) => (first - 0.4, 245.7),
                (_, Some(n)) => (first - 0.4, n + 2.0),
                _ => (first - 0.4, line.end() + 4.0),
            };
            let group = text_scene.add(Node::group().with_position(spec.station.pos).with_rotation(spec.station.rot).hidden());

            if lrc.is_some_and(|i| CLUSTER.contains(&i)) {
                let mut arrivals = Vec::new();
                let mut words = Vec::new();
                for (wi, word) in line.words.iter().enumerate() {
                    let style = lrc.and_then(|i| WORD_STYLES.iter().find(|s| s.0 == i && s.1 == word.text.as_str()));
                    let scale = style.map_or(1.0, |s| s.2) * 1.25;
                    let color = Rgb::hex(style.and_then(|s| s.3).unwrap_or(TEXT));
                    let boxed = lrc.is_some_and(|i| BOXED.contains(&(i, word.text.as_str())));
                    let s = spec.size * scale;
                    let chars: Vec<(char, Arc<Geometry>, f32)> =
                        word.text.chars().filter_map(|c| glyph_in(false, c).map(|(g, a)| (c, g, a))).collect();
                    let total: f32 = chars.iter().map(|c| c.2 * s).sum();
                    let mut pen = -total * 0.5;
                    let theta = 0.7 + wi as f32 * 2.39996 + (hash3(li as u32, wi as u32) - 0.5) * 0.5;
                    for (n, (_, geo, adv)) in chars.iter().enumerate() {
                        let base = Vec3::new(pen, -s * 0.36, 0.0);
                        pen += adv * s;
                        let glyph_color = Rgb::hex(TEXT);
                        let node = text_scene.add_to(
                            group,
                            Node::mesh(geo.clone(), Material::unlit(glyph_color).transparent().opacity(0.0))
                                .with_position(base)
                                .scaled(s)
                                .hidden(),
                        );
                        let plate = boxed.then(|| aura(&mut text_scene, node, geo, *adv));
                        glyphs.push(GlyphInst {
                            kind: Kind::Main,
                            plate,
                            tilt: (hash3(li as u32 * 13 + wi as u32, n as u32) - 0.5) * 0.12,
                            scatter: Vec3::ZERO,
                            node,
                            line: li,
                            appear: word.start - 0.12,
                            word_start: word.start,
                            word_end: word.end,
                            base,
                            color: glyph_color,
                            plate_color: color,
                            slam: style.is_some_and(|s| s.4),
                            cluster: Some((wi, theta)), accent: None, width: 0.0, ch: ' ',
                        });
                    }
                    arrivals.push(word.start);
                    words.push((0.5 * (word.start + word.end), Vec3::ZERO));
                }
                line_insts.push(LineInst { arrivals, group, spec: *spec, in_t, out_t, start: first, end: line.end(), words });
                continue;
            }

            // Rows of (word, char, scale, colour, slam).
            let mut rows: Vec<Vec<(usize, char, f32, Rgb, bool)>> = vec![vec![]];
            for (wi, word) in line.words.iter().enumerate() {
                let style = lrc.and_then(|i| WORD_STYLES.iter().find(|s| s.0 == i && s.1 == word.text.as_str()));
                let emph = lrc.is_some_and(|i| BOXED.contains(&(i, word.text.as_str())));
                let scale = style.map_or(1.0, |s| s.2) * if emph { 1.12 } else { 1.0 };
                let color = Rgb::hex(style.and_then(|s| s.3).unwrap_or(TEXT));
                let slam = style.is_some_and(|s| s.4) || lrc.is_none();
                for ch in word.text.chars() {
                    // Quote marks stay small next to an enlarged word.
                    let scale = if ch == '"' { scale.min(1.1) } else { scale };
                    rows.last_mut().unwrap().push((wi, ch, scale, color, slam));
                }
                let forced = lrc.is_some_and(|i| BREAKS.contains(&(i, word.text.as_str())));
                if (word.break_after || forced) && wi + 1 < line.words.len() {
                    rows.push(vec![]);
                }
            }

            let size = spec.size;
            let track = if lrc.is_none() { 0.35 * size } else { 0.04 * size };
            let row_gap = 1.38;
            let mut word_acc: HashMap<usize, (Vec3, f32)> = HashMap::new();
            let n_rows = rows.len() as f32;
            for (ri, row) in rows.iter().enumerate() {
                let advances: Vec<f32> =
                    row.iter().map(|(_, ch, sc, _, _)| glyph_in(false, *ch).map_or(0.0, |g| g.1) * size * sc).collect();
                let tallest = row.iter().map(|r| r.2).fold(1.0_f32, f32::max) * size;
                let length: f32 = match spec.flow {
                    Flow::Vertical => row.iter().map(|r| size * r.2 * 1.06).sum(),
                    _ => advances.iter().map(|a| a + track).sum::<f32>() - track,
                };
                let (col_x, row_y) = match spec.flow {
                    Flow::Vertical => (-(ri as f32) * size * 1.7 + (n_rows - 1.0) * size * 0.85, 0.0),
                    Flow::Horizontal => {
                        (0.0, -(ri as f32) * size * row_gap + (n_rows - 1.0) * size * row_gap * 0.5 - tallest * 0.35)
                    }
                    Flow::Stack => (0.0, ri as f32 * size * 1.9 - tallest * 0.35),
                    Flow::Punch | Flow::PunchBack => (0.0, -tallest * 0.35),
                };
                let mut pen = match spec.flow {
                    Flow::Vertical => length * 0.5,
                    _ => -length * spec.align,
                };
                let mut nth = 0;
                let mut prev_word = usize::MAX;
                for (k, (wi, ch, sc, color, slam)) in row.iter().enumerate() {
                    if *wi != prev_word {
                        nth = 0;
                        prev_word = *wi;
                    }
                    let Some((geo, adv)) = glyph_in(false, *ch) else { continue };
                    let s = size * sc;
                    let base = match spec.flow {
                        Flow::Vertical => Vec3::new(col_x - adv * s / 2.0, pen - s * 0.88, 0.0),
                        Flow::Punch => Vec3::new(pen, row_y, -(ri as f32) * PUNCH_D),
                        Flow::PunchBack => Vec3::new(pen, row_y, ri as f32 * PUNCH_D),
                        _ => Vec3::new(pen, row_y, 0.0),
                    };
                    // ...and hang from the top of the word they mark.
                    let word_sc = WORD_STYLES.iter().find(|w| Some(w.0) == *lrc && w.1 == line.words[*wi].text).map_or(1.0, |w| w.2);
                    let base = if *ch == '"' { base + Vec3::new(0.0, size * (word_sc - sc) * 0.8, 0.0) } else { base };
                    pen += match spec.flow {
                        Flow::Vertical => -s * 1.06,
                        _ => advances[k] + track,
                    };
                    let centre = base + Vec3::new(adv * s * 0.5, s * 0.36, 0.0);
                    let acc = word_acc.entry(*wi).or_insert((Vec3::ZERO, 0.0));
                    acc.0 += centre;
                    acc.1 += 1.0;
                    let word = &line.words[*wi];
                    let boxed = lrc.is_some_and(|i| BOXED.contains(&(i, word.text.as_str())));
                    let h = |k: u32| hash3(li as u32 * 977 + k, glyphs.len() as u32);
                    let tilt = if boxed { deg((h(1) - 0.5) * 7.0) } else { 0.0 };
                    let accent = lrc.and_then(|i| WORD_STYLES.iter().find(|s| s.0 == i && s.1 == word.text.as_str())).and_then(|s| s.3);
                    let glyph_color = Rgb::hex(TEXT);
                    let node = text_scene.add_to(
                        group,
                        Node::mesh(geo.clone(), Material::unlit(glyph_color).transparent().opacity(0.0))
                            .with_position(base)
                            .scaled(s)
                            .hidden(),
                    );
                    // Emphasis: a soft aura round the letter (layered glow copies), no shake.
                    let plate = (boxed || accent.is_some()).then(|| aura(&mut text_scene, node, &geo, adv));
                    let assemble = lrc.is_some_and(|i| ASSEMBLE.contains(&i));
                    let scatter = if assemble {
                        Vec3::new((h(2) - 0.5) * 14.0, (h(3) - 0.5) * 8.0, 2.0 + h(4) * 6.0)
                    } else {
                        Vec3::ZERO
                    };
                    let appear = word.start - 0.12 + nth as f32 * 0.036 + (h(5) - 0.5) * 0.022 - if assemble { 0.35 } else { 0.0 };
                    glyphs.push(GlyphInst {
                        kind: Kind::Main,
                        plate,
                        tilt,
                        scatter,
                        node,
                        line: li,
                        appear,
                        word_start: word.start,
                        word_end: word.end,
                        base,
                        color: glyph_color,
                        plate_color: *color,
                        cluster: None,
                        accent,
                        width: adv * s,
                        ch: *ch,
                        slam: *slam || (spec.entrance == Entrance::Slam && nth == 0 && *wi == 0),
                    });
                    // 何度も繰り返し: receding copies.
                    if lrc.is_some_and(|i| ECHO.contains(&i)) {
                        for n in 1..=4u8 {
                            let e = text_scene.add_to(
                                group,
                                Node::mesh(geo.clone(), Material::unlit(glyph_color).transparent().opacity(0.0))
                                    .with_position(base)
                                    .scaled(s)
                                    .hidden(),
                            );
                            glyphs.push(GlyphInst {
                                kind: Kind::Echo(n),
                                plate: None,
                                tilt: 0.0,
                                scatter: Vec3::ZERO,
                                node: e,
                                line: li,
                                appear: appear + 0.12 * n as f32,
                                word_start: word.start,
                                word_end: word.end,
                                base: base + Vec3::new(0.0, 0.0, -0.45 * n as f32),
                                color: glyph_color,
                                plate_color: *color,
                                cluster: None, accent: None, width: 0.0, ch: ' ',
                                slam: false,
                            });
                        }
                    }
                    nth += 1;
                }
            }
            // The title: a dark knockout behind each letter so the circle's lines don't run
            // through it, a spaced Latin subtitle and the cover's red rule.
            if lrc.is_none() {
                let title_glyphs: Vec<usize> = (0..glyphs.len()).filter(|&k| glyphs[k].line == li).collect();
                for k in title_glyphs {
                    let node = glyphs[k].node;
                    let geo = match &text_scene[node].object {
                        fframes::three_d::Object::Mesh(m) => m.geometry.clone(),
                        _ => continue,
                    };
                    let centre = Vec3::new(0.4, 0.36, 0.0);
                    let knock = text_scene.add_to(
                        node,
                        Node::mesh(geo, Material::unlit(Rgb::hex(0x0a0712)).transparent().opacity(0.0))
                            .with_position(centre * (1.0 - 1.35) + Vec3::Z * -0.12)
                            .scaled(1.35),
                    );
                    glyphs[k].plate = Some(knock);
                }
                let sub = "I L I O S";
                let rs = 0.34;
                let advs: Vec<f32> = sub.chars().map(|c| glyph_in(true, c).map_or(0.0, |g| g.1) * rs + rs * 0.35).collect();
                let total: f32 = advs.iter().sum();
                let mut pen = -total * 0.5;
                for (k, c) in sub.chars().enumerate() {
                    if let Some((geo, _)) = glyph_in(true, c) {
                        let node = text_scene.add_to(group, Node::mesh(geo, Material::unlit(Rgb::hex(0xe4d2d7)).transparent().opacity(0.0))
                            .with_position(Vec3::new(pen, -size * 0.95, 0.0))
                            .scaled(rs)
                            .hidden());
                        let at = BAND_IN + 4.0 * BEAT + k as f32 * 0.05;
                        glyphs.push(GlyphInst { kind: Kind::Main, plate: None, tilt: 0.0, scatter: Vec3::ZERO, node, line: li, appear: at,
                            word_start: at, word_end: at + 0.5, base: Vec3::new(pen, -size * 0.95, 0.0), color: Rgb::hex(0xe4d2d7),
                            plate_color: Rgb::hex(0xe4d2d7), slam: false, cluster: None, accent: None, width: 0.0, ch: ' ' });
                    }
                    pen += advs[k];
                }
                let rule = text_scene.add_to(group, Node::mesh(Geometry::cuboid(1.0, 0.035, 0.01), Material::unlit(Rgb::hex(0xff5a6e)).transparent().opacity(0.0))
                    .with_position(Vec3::new(0.0, -size * 0.66, 0.0))
                    .hidden());
                let at = BAND_IN + 4.0 * BEAT + 0.3;
                glyphs.push(GlyphInst { kind: Kind::Main, plate: None, tilt: 0.0, scatter: Vec3::ZERO, node: rule, line: li, appear: at,
                    word_start: at, word_end: at + 0.5, base: Vec3::new(0.0, -size * 0.66, 0.0), color: Rgb::hex(0xff5a6e),
                    plate_color: Rgb::hex(0xff5a6e), slam: false, cluster: None, accent: None, width: 0.0, ch: ' ' });
            }
            let mut words: Vec<(f32, Vec3)> = line
                .words
                .iter()
                .enumerate()
                .filter_map(|(wi, w)| word_acc.get(&wi).map(|(c, n)| (0.5 * (w.start + w.end), *c / *n)))
                .collect();
            words.sort_by(|a, b| a.0.total_cmp(&b.0));
            line_insts.push(LineInst { arrivals: Vec::new(), group, spec: *spec, in_t, out_t, start: first, end: line.end(), words });
        }

        // メロディア: a staff runs behind the word and the notes slide along it, like reading the score.
        let mut staffs = Vec::new();
        for (li, t0, t1) in [(8usize, 68.5_f32, 70.2_f32), (21, 151.3, 153.0)] {
            let Some(l) = line_insts.get(li) else { continue };
            let size = l.spec.size;
            let base = Vec3::new(0.0, 0.0, -2.0 * PUNCH_D - 0.6);
            let g = text_scene.add_to(l.group, Node::group().with_position(base).hidden());
            let mat = || Material::unlit(Rgb::hex(TEXT)).transparent().opacity(0.0);
            // Everything on the staff rides one wave: (node, x, rest y).
            let mut parts = Vec::new();
            for k in 0..5 {
                let y = (k as f32 - 2.0) * 0.36 * size;
                for sgm in 0..150 {
                    let x = -45.0 + sgm as f32 * 0.6;
                    parts.push((text_scene.add_to(g, Node::mesh(Geometry::cuboid(0.66, 0.02, 0.01), mat()).at(x, y, 0.0)), x, y));
                }
            }
            for n in 0..30u32 {
                let x = -28.0 + n as f32 * 2.1 + hash3(n, 5) * 0.9;
                let y = ((hash3(n, li as u32) * 9.0).floor() - 4.0) * 0.18 * size;
                let head = text_scene.add_to(g, Node::mesh(Geometry::circle(0.17 * size, 20), mat()).at(x, y, 0.0).with_rotation(Quat::from_rotation_z(0.4)));
                text_scene[head].scale = Vec3::new(1.3, 0.85, 1.0);
                parts.push((head, x, y));
                let up = if y < 0.0 { 1.0 } else { -1.0 };
                let stem = text_scene.add_to(g, Node::mesh(Geometry::cuboid(0.03, 1.1 * size, 0.01), mat()).at(x + up * 0.2 * size, y + up * 0.55 * size, 0.0));
                parts.push((stem, x, y + up * 0.55 * size));
            }
            staffs.push((g, base, t0, t1, size, parts));
        }
        let mut angel_scene = Scene::new();
        let angel_group = angel_scene.add(Node::group().hidden());
        let mut angel = Vec::new();
        {
            let size = 0.42;
            let track = 0.1 * size;
            let letters: Vec<(char, f32)> =
                ANGEL.iter().flat_map(|(w, at)| w.chars().enumerate().map(move |(k, c)| (c, at + 0.07 * k as f32))).collect();
            let total: f32 = letters.iter().map(|(c, _)| glyph_in(false, *c).map_or(0.0, |g| g.1) * size + track).sum::<f32>() - track;
            let mut pen = -total * 0.5;
            for (c, at) in letters {
                let Some((geo, adv)) = glyph_in(false, c) else { continue };
                let base = Vec3::new(pen, -size * 0.36, 0.0);
                let node = angel_scene.add_to(
                    angel_group,
                    Node::mesh(geo, Material::unlit(Rgb::hex(TEXT)).transparent().opacity(0.0)).with_position(base).scaled(size),
                );
                angel.push((node, base, at));
                pen += adv * size + track;
            }
        }
        let hash = |i: u32, s: u32| -> f32 {
            let mut x = i.wrapping_mul(0x9E3779B1) ^ s.wrapping_mul(0x85EBCA77);
            x ^= x >> 15;
            x = x.wrapping_mul(0x2C1B3C6D);
            x ^= x >> 12;
            (x & 0xffff) as f32 / 65535.0
        };
        let dust = (0..700)
            .map(|i| {
                (
                    Vec3::new(hash(i, 1) * 40.0 - 20.0, hash(i, 2) * 40.0 - 18.0, hash(i, 3) * 40.0 - 16.0),
                    0.01 + hash(i, 4) * 0.025,
                    0.05 + hash(i, 5) * 0.1,
                    0.2 + hash(i, 6) * 0.6,
                )
            })
            .collect();

        Self {
            scene,
            renderer: Renderer::new(width, height)
                .tone_mapping(ToneMapping::Neutral)
                .bloom(Bloom { strength: 0.45, radius: 0.5, threshold: 1.0 }),
            text_scene,
            text_renderer: Renderer::new(width, height).tone_mapping(ToneMapping::Linear),
            graphics,
            glyphs,
            lines: line_insts,
            staffs,
            dust,
            angel_scene,
            angel_renderer: Renderer::new(width, height).tone_mapping(ToneMapping::Linear),
            angel: (angel_group, angel),
        }
    }

    /// Camera for line `i` (0 = title) at time `t`: it looks at the word being sung (a
    /// smoothed average of the word centres around `t`) from in front of the station.
    fn line_pose(&self, i: usize, t: f32) -> CamPose {
        if i == 13 {
            // 向こうへと向かうんだ: the rush cuts to stillness. The words stand huge and far off,
            // seen from below and to one side, and the camera drifts slowly round them.
            let st = self.lines[13].spec.station;
            let s = (t - 90.1).max(0.0);
            let th = -0.62 + 0.9 * (1.0 - (-s / 9.0).exp());
            let r = 30.0 + 1.2 * s;
            let pos = st.pos + Vec3::new(th.sin() * r, -5.0 + 0.2 * s, th.cos() * r);
            return CamPose { pos, look: st.pos + Vec3::new(0.0, 0.6, 0.0), fov: 44.0, up: Vec3::Y };
        }
        let line = &self.lines[i];
        let spec = &line.spec;
        let tc = t.clamp(line.start - 0.3, line.end + 0.6);
        let mut acc = Vec3::ZERO;
        let mut wsum = 0.0;
        for (wt, c) in &line.words {
            let w = (-((tc - wt) / 0.55).powi(2)).exp() + 1e-4;
            acc += *c * w;
            wsum += w;
        }
        let centre = line.words.iter().map(|w| w.1).sum::<Vec3>() / line.words.len().max(1) as f32;
        // Keep a little of the line's centre so the camera doesn't chase every syllable.
        // While time stops (届くはずのない…) it holds dead centre.
        let punch = matches!(spec.flow, Flow::Punch | Flow::PunchBack);
        let target = if i == 10 || i == 23 { centre } else if punch { acc / wsum } else { (acc / wsum).lerp(centre, 0.25) };
        // While a note is held the letter grows: stay on the whole line and give it room.
        let held = HOLDS.iter().filter(|h| h.0 == i).map(|h| ease_between(t, h.3, h.4) * (1.0 - ease_between(t, h.4, h.4 + 0.8))).fold(0.0_f32, f32::max);
        let held = if i >= 38 { 0.0 } else { held };
        let target = target.lerp(centre, 0.6 * held);

        // Slow push while the line is sung.
        let u = clamp01((t - line.start) / (line.end - line.start + 1.0).max(1.0));
        let mut dist = spec.shot.dist * (1.0 - 0.14 * u) * (1.0 - 0.2 * held);
        if i == 3 {
            dist *= 1.0 - 0.3 * ease_out((t - 43.68) / 0.35); // 起きる: snap in
        }
        if i == 9 {
            // The last く of 寂しく: drag the camera out of the situation, hard.
            dist *= 1.0;
        }
        if i == 22 {
            dist *= 1.0;
        }
        // 迎えるんだ: the camera backs away, receiving rather than going.
        if i == 25 {
            dist *= 1.0 + 1.6 * expo_out((t - 168.46) / 3.5);
        }
        if i == 19 {
            // 見てーみたーかーった: back off a little on the first held note, then far and fast on the long one.
            dist *= 1.0 - 0.25 * ease_in((t - 140.05) / 4.3);
        }
        let off = Vec3::new(-spec.shot.ox, -spec.shot.oy, 0.0);
        let anchor = spec.station.world(target + off);
        let face = spec.station.rot * Vec3::Z;
        let mut up = spec.station.rot * Vec3::Y;
        // The shot never sits square on: it keeps swinging slowly round the words.
        let c = world_clock(t);
        let swing = Quat::from_axis_angle(up, 0.10 * (c * 0.21 + i as f32 * 1.7).sin())
            * Quat::from_axis_angle(spec.station.rot * Vec3::X, 0.05 * (c * 0.17 + i as f32).sin());
        // Held notes: the camera pushes in and swings out to the side while the lens widens,
        // so the words hold their size and the space behind them stretches away.
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let swing = Quat::from_axis_angle(up, 0.42 * held * side) * Quat::from_axis_angle(spec.station.rot * Vec3::X, -0.12 * held) * swing;
        let normal = if i == 12 { face } else { swing * face };
        if i == 19 {
            // ...and the horizon turns under it the whole time.
            up = Quat::from_axis_angle(face, 0.12 * ease_between(t, 138.3, 139.7) + 0.38 * ease_between(t, 140.05, 144.3)) * up;
        }
        let mut fov = spec.shot.fov;
        let mut look = anchor;
        fov += 11.0 * held;
        // 置いて: we are the one leaving. The camera lifts away and the words stay where they are,
        // smaller and smaller.
        if i == 38 {
            let x = ease_in((t - 241.0) / 4.4);
            dist *= 1.0 + 4.0 * x;
            up = Quat::from_axis_angle(face, 0.3 * x) * up;
        }
        if i == 39 {
            // One retreat, never forward: it carries on from where 置いて left the camera (the
            // speed of that pull-away bleeds off, it does not stop), then the long pull back.
            let end = 6.6 * 0.86 * 5.0;
            let s = (t - 245.4).max(0.0);
            let x = smooth5(clamp01((t - 248.2) / 8.5));
            dist = end + 9.3 * (1.0 - (-s / 0.6).exp()) + 1.5 * s + 58.0 * x - LAST_GAP;
            // Look down the axis, not at the line: early in the move the line is still behind us.
            look = anchor - normal * 40.0;
            up = Quat::from_axis_angle(face, -0.5 * x) * up;
        }
        if (32..=34).contains(&i) {
            fov += 14.0 * ease_in((t - 202.0) / 17.0);
        }
        if i == 37 {
            // 行ってしまう: the eye goes with the light that leaves, as far as it can.
            let k = self.kimi_light(t);
            look += (k - look).clamp_length_max(7.0) * ease_between(t, 231.6, 234.6);
            up = Quat::from_axis_angle(face, 0.3 * ease_between(t, 232.0, 235.0)) * up;
        }
        // 笑顔で言う: fall into the text and on through the tunnel behind it.
        if i == 6 {
            let w = wormhole(t);
            dist *= 1.0 - 1.9 * w;
            look = anchor - normal * 8.0 * w;
            fov += 55.0 * w * w;
            up = Quat::from_axis_angle(normal, 1.6 * w * w) * up;
        }
        // Clusters: the camera breathes and rolls a little, YOASOBI-style.
        if !line.arrivals.is_empty() {
            up = Quat::from_axis_angle(normal, 0.1 * (t * 0.7 + i as f32).sin()) * up;
            dist *= 1.0 + 0.06 * (t * 0.9).sin();
        }
        if i == 9 {
            fov += 5.0 * ease_between(t, 73.2, 75.4);
        }
        if i == 22 {
            fov += 5.0 * ease_between(t, 156.0, 158.0);
        }
        // 青い世界: start looking up into the light, then tilt down onto the words.
        if i == 5 {
            look += up * 3.2 * (1.0 - expo_out((t - BLUE_WORLD + 0.3) / 1.8));
        }
        // The last だ of 抱えて向かうんだ: rush forward through it.
        if i == 12 {
            // Builds over the whole held note, then plunges through the letter.
            let x = clamp01((t - 87.85) / 2.25);
            let rush = x * x * (0.35 + 0.65 * x);
            dist -= 11.0 * rush;
            fov += 38.0 * rush;
            // Keep looking ahead, past the letter, so we fly through it instead of turning.
            look = anchor - normal * 14.0 * rush;
            up = Quat::from_axis_angle(normal, 0.5 * rush * rush) * up;
        }
        CamPose { pos: anchor + normal * dist, look, fov, up }
    }

    /// A line's camera plus the long moves that happen after it: drifting away when the
    /// band drops after chorus 1, and the final pull back over everything that was sung.
    fn held_pose(&self, i: usize, t: f32) -> CamPose {
        let mut pose = self.line_pose(i, t);
        if i == 13 {
            // Interlude: two small lights come out of the dark and we follow them up to where
            // the second verse is.
            let c = self.lights_path(t);
            let phi = 0.5 + (t - 95.0) * 0.05;
            let chase = CamPose { pos: c + Vec3::new(phi.sin() * 6.5, 1.2, phi.cos() * 6.5), look: c, fov: 40.0, up: Vec3::Y };
            pose = blend(&pose, &chase, smooth5(clamp01((t - 95.5) / 6.5)), false, 0.35, 0.0);
        }
        if i == self.lines.len() - 1 {
            // After the last word: lift the eyes from the ground to the horizon. The glide starts
            // where the retreat ends and keeps drifting backwards, so the camera never returns.
            let d = (t - 253.5).max(0.0);
            let gpos = Vec3::new(0.6 * (d * 0.11).sin(), 60.0 + 0.10 * d, -600.0 + 0.9 * d);
            let glide = CamPose { pos: gpos, look: gpos + Vec3::new(0.02, -0.02, -1.0) * 60.0, fov: 42.0, up: Vec3::Y };
            pose = blend(&pose, &glide, smooth5(clamp01((t - 253.5) / 12.0)), false, 0.25, 0.0);
        }
        pose
    }

    /// 君's light in the last chorus. It rises out of 君 as the word is sung, hops along the line
    /// to each word in time with the voice, like the ball over a sing-along lyric, and from the
    /// last word it leaves toward the place the last star will be.
    fn kimi_light(&self, t: f32) -> Vec3 {
        let line = &self.lines[37];
        let (st, size) = (line.spec.station, line.spec.size);
        // (arrival, resting place): inside 君 first, then just above each later word.
        let stops: Vec<(f32, Vec3)> = line
            .words
            .iter()
            .enumerate()
            .map(|(k, (mid, c))| (mid - 0.22, *c + Vec3::new(0.0, if k == 0 { 0.0 } else { 0.62 * size }, 0.4)))
            .collect();
        let Some(&(last_at, last)) = stops.last() else { return st.world(Vec3::ZERO) };
        let mut p = stops[0].1;
        for pair in stops.windows(2) {
            let ((_, from), (at, to)) = (pair[0], pair[1]);
            let hop = (at - pair[0].0).clamp(0.05, 0.42);
            let u = clamp01((t - (at - hop)) / hop);
            if u > 0.0 {
                p = from.lerp(to, smooth5(u)) + Vec3::Y * 0.4 * size * (u * PI).sin();
            }
        }
        if std::env::var_os("ILIOS_CAM_DEBUG").is_some() && (t - 230.0).abs() < 0.02 {
            eprintln!("KIMI stops={:?}", stops);
        }
        let _ = last;
        st.world(p) + KIMI_DIR.normalize() * 600.0 * ease_in((t - last_at - 0.35) / 4.6)
    }

    /// Where the two lights of the interlude are heading: from ahead of the camera after
    /// chorus 1 up to the first line of verse 2.
    fn lights_path(&self, t: f32) -> Vec3 {
        let from = self.line_pose(13, 95.0);
        let a = from.pos + (from.look - from.pos).normalize() * 16.0;
        let b = Vec3::new(0.0, 11.4, -274.0);
        let x = smooth5(clamp01((t - 95.0) / 14.5));
        a.lerp(b, x) + Vec3::Y * 3.0 * (x * PI).sin()
    }

    pub fn camera_at(&self, t: f32) -> CamPose {
        let first_lyric = &self.lines[1];
        let mut pose = if t < first_lyric.start - 0.35 {
            let intro = intro_pose(t);
            // Zoom through the title into the first verse line.
            let (a, b) = (26.6, first_lyric.start - 0.35);
            if t > a { blend(&intro, &self.line_pose(1, t), whip((t - a) / (b - a + SETTLE)), false, 0.3, 0.0) } else { intro }
        } else {
            let n = self.lines.len();
            let i = (1..n).rev().find(|&i| t >= self.lines[i].start - 0.12).unwrap_or(1);
            // A move covers [land - whip, land + SETTLE]: it reaches ~85% on the landing and
            // keeps gliding into the shot afterwards, so the camera never stops dead.
            let window = |j: usize| {
                // Moves start early and take their time; a 0 stays a hard cut.
                let w = self.lines[j].spec.shot.whip;
                let w = if w > 0.0 { w.max(0.7) * 1.5 } else { 0.0 };
                let land = self.lines[j].start - 0.12;
                if j == 11 || j == 24 {
                    // Out of the time stop: nothing moves until time does.
                    let s0 = TIME_STOPS[if j == 11 { 0 } else { 1 }].1 - 0.05;
                    return (w, s0, land + SETTLE - s0);
                }
                if w <= 0.0 {
                    return (w, land, SETTLE);
                }
                // Never leave a line before its last word has been sung, or while a note is held.
                let prev = &self.lines[j - 1];
                let w = if matches!(self.lines[j].spec.flow, Flow::Punch | Flow::PunchBack) { 0.45 } else { w };
                let hold_end = HOLDS.iter().filter(|h| h.0 == j - 1).map(|h| h.4).fold(0.0_f32, f32::max);
                let earliest = prev.words.last().map_or(0.0, |w| w.0).max(hold_end - 0.3);
                let start = (land - w).max(earliest).min(land - 0.3);
                (w, start, ((land - start) / 0.5).clamp(1.2, 5.0))
            };
            let mut pose = self.held_pose(i, t);
            if i > 1 {
                let (w, start, len) = window(i);
                if w > 0.0 && t < start + len {
                    let arc = self.lines[i].spec.arc && self.lines[i - 1].spec.arc;
                    let (r, n) = move_roll(i);
                    pose = blend(&self.held_pose(i - 1, t), &pose, whip((t - start) / len), arc, r, n);
                }
            }
            let next = i + 1;
            if next < n {
                let (w, start, len) = window(next);
                if w > 0.0 && t > start {
                    let arc = self.lines[next].spec.arc && self.lines[i].spec.arc;
                    let (r, n) = move_roll(next);
                    pose = blend(&pose, &self.line_pose(next, t), whip((t - start) / len), arc, r, n);
                }
            }
            pose
        };

        // Floating: a slow, wide drift and a sway of the horizon that never stop (they run on
        // the world clock, so they freeze with it). Strongest in the intro and the quiet parts.
        {
            let c = world_clock(t);
            let amt = 1.0 + 1.2 * (1.0 - ease_between(t, BAND_IN - 2.0, VERSE_1)) + 0.8 * window_f(t, INTERLUDE, VERSE_2, 2.0)
                + 0.8 * ease_between(t, 248.0, 256.0);
            let cam = pose.camera();
            let wob = |a: f32, b: f32, d: f32| (c * a).sin() * 0.6 + (c * b + 1.7).sin() * 0.3 + (c * d + 4.1).sin() * 0.1;
            let fwd = cam.forward();
            let right = fwd.cross(cam.up_vector()).normalize_or_zero();
            let drift = right * wob(0.29, 0.71, 1.9) + cam.up_vector() * wob(0.23, 0.61, 2.1) + fwd * 0.6 * wob(0.17, 0.43, 1.3);
            pose.pos += drift * 0.2 * amt;
            pose.look += drift * 0.07 * amt;
            let sway = (0.03 * (c * 0.37).sin() + 0.018 * (c * 0.83 + 1.0).sin()) * amt;
            pose.up = Quat::from_axis_angle(fwd, sway) * pose.up;
        }

        // Impacts: a short shake and a punch-in.
        let hit = impact(t, 0.16);
        if hit > 0.001 {
            let n = |s: f32| ((t * 61.0 + s).sin() * 43758.545).fract() - 0.5;
            let cam = pose.camera();
            let shake = cam.right() * n(1.3) + cam.up_vector() * n(7.1);
            pose.pos += shake * 0.12 * hit;
            pose.look += shake * 0.05 * hit;
        }
        pose.fov -= 3.0 * impact(t, 0.3).min(1.0);
        // Flight: the lens opens and the frame starts to tremble as it builds.
        let w = warp(t);
        if w > 0.0 {
            let n = |s: f32| ((t * 47.0 + s).sin() * 43758.545).fract() - 0.5;
            let cam = pose.camera();
            let shake = (cam.forward().cross(cam.up_vector()) * n(2.1) + cam.up_vector() * n(5.3)) * 0.05 * w * w;
            pose.pos += shake;
            pose.look += shake;
            pose.fov += 15.0 * w;
        }
        pose.fov -= 1.8 * beat_pulse(t) + 0.7 * cue_pulse(SNARE, t, 0.12);
        pose
    }

    /// The second voice's 好きだった, like a guardian over the one who sings. It comes down out
    /// of the light above the frame and stays with the camera as it moves on to 「月」: a halo
    /// over the words, two wings of light that open round the frame, a few feathers falling.
    /// As the next line begins it lifts away.
    fn pose_angel(&self, ts: &mut Scene, debris: &mut Vec<Particle>, t: f32, cam: &CamPose) -> f32 {
        let (group, letters) = &self.angel;
        let (a0, a1) = (ANGEL[0].1 - 0.3, BRIDGE_MOON + 0.6);
        let on = window_f(t, a0, a1, 0.6);
        ts[*group].visible = on > 0.003;
        if !ts[*group].visible {
            return 0.0;
        }
        let camera = cam.camera();
        let fwd = camera.forward();
        let right = fwd.cross(camera.up_vector()).normalize_or_zero();
        let up = right.cross(fwd);
        let rot = Quat::from_mat3(&Mat3::from_cols(right, up, -fwd));
        // Six metres ahead, scaled with the lens so it keeps its size on screen through the move.
        let scale = (deg(cam.fov) * 0.5).tan() / deg(22.0).tan();
        let leave = ease_in(clamp01((t - BRIDGE_MOON + 0.2) / 0.9));
        let lift = 1.05 - 0.10 * (t - ANGEL[0].1) + 0.9 * leave;
        // It glides in from the upper right, clear of その目が, and settles over the middle.
        let glide = 1.5 * (1.0 - smooth5(clamp01((t - ANGEL[0].1 + 0.2) / 1.3)));
        let centre = cam.pos + fwd * 6.0 + rot * Vec3::new(glide, 0.35 * glide, 0.0) * scale;
        ts[*group].position = centre;
        ts[*group].rotation = rot;
        ts[*group].scale = Vec3::splat(scale);
        let world = |p: Vec3| centre + rot * (p * scale);
        let gold = Rgb::hex(ANGEL_GOLD);

        for (k, (node, base, at)) in letters.iter().enumerate() {
            let since = t - at;
            let d = expo_out(since / 1.1);
            let sway = 0.025 * (t * 1.3 + k as f32 * 0.9).sin();
            // Each letter floats down out of the light, stretched tall, and settles.
            let p = *base + Vec3::new(sway, lift + 0.45 * (1.0 - d) + 0.25 * leave * hash3(k as u32, 41), 0.0);
            let opacity = expo_out(since / 0.4) * on * (1.0 - leave);
            let n = &mut ts[*node];
            n.visible = since > 0.0 && opacity > 0.002;
            n.position = p;
            n.scale = Vec3::new(0.42 * (0.9 + 0.1 * d), 0.42 * (1.0 + 0.5 * (1.0 - d)), 0.42);
            if let Some(m) = n.material_mut() {
                // Only the coverage matters: the shader gives it its colour and haze.
                m.opacity = opacity * (0.75 + 0.25 * (-(since.max(0.0)) / 1.0).exp());
            }
        }

        let born = expo_out((t - ANGEL[0].1 + 0.25) / 0.9) * on;
        // The halo: a soft ring of light over the words, seen a little from below, with a
        // brighter glint running round it.
        let halo_c = Vec3::new(0.0, lift + 0.62, -0.3);
        let r = 0.5 * (0.7 + 0.3 * born);
        for k in 0..120 {
            let a = TAU * k as f32 / 120.0;
            let glint = (0.5 + 0.5 * (a - t * 1.8).cos()).powi(6);
            let p = halo_c + Vec3::new(a.cos() * r, a.sin() * r * 0.22, a.sin() * 0.25);
            let amt = 0.6 * born * (1.0 - leave) * (0.35 + 0.65 * glint);
            debris.push(Particle::new(world(p), 0.012 * scale, Rgb::hex(0xffffff).lerp(gold, 0.25) * (0.9 * amt)).opacity(amt.min(1.0)));
            debris.push(Particle::new(world(p), 0.06 * scale, gold * (0.12 * amt)).opacity((0.35 * amt).min(1.0)));
            debris.push(Particle::new(world(p), 0.2 * scale, gold * (0.02 * amt)).opacity((0.1 * amt).min(1.0)));
        }
        // Wings: from either side of the words an arm of light rises and reaches out, and
        // feathers hang from it, longest at the tip. They open from folded until they span the
        // frame, as if to shelter it. Faint: a shape in the light, not a drawing.
        let unfold = smooth5(clamp01((t - ANGEL[0].1 - 0.1) / 1.8));
        let wings = 0.7 * born * (1.0 - leave) * (0.85 + 0.15 * (t * 1.7).sin());
        let beat = 0.06 * (t * 2.1).sin();
        for side in [-1.0_f32, 1.0] {
            let reach = 0.3 + 0.7 * unfold;
            // The arm: shoulder, up to the wrist, out and down to the tip.
            let arm = |u: f32| {
                let x = 0.7 + 2.6 * u * reach;
                let y = lift + 0.05 + (0.95 + beat) * (u * PI * 0.8).sin() * (0.6 + 0.4 * reach) - 0.35 * u * u;
                Vec3::new(x * side, y, -1.0 - 0.6 * u)
            };
            let mut stroke = |a: Vec3, b: Vec3, amt: f32| {
                let steps = (((b - a).length() / 0.012) as usize).clamp(2, 400);
                for s in 0..steps {
                    let x = (s as f32 + 0.5) / steps as f32;
                    let p = world(a.lerp(b, x));
                    let k = amt * (x * PI).sin().powf(0.5);
                    debris.push(Particle::new(p, 0.022 * scale, Rgb::hex(0xffffff).lerp(gold, 0.4) * (0.2 * k)).opacity((0.6 * k).min(1.0)));
                    debris.push(Particle::new(p, 0.1 * scale, gold * (0.02 * k)).opacity((0.2 * k).min(1.0)));
                }
            };
            for k in 0..24 {
                stroke(arm(k as f32 / 24.0), arm((k + 1) as f32 / 24.0), 1.3 * wings);
            }
            for f in 0..15 {
                let u = (f as f32 + 0.5) / 15.0;
                let root = arm(u);
                // Each feather hangs down and sweeps outward more toward the tip.
                let ang = deg(-8.0 - 52.0 * u) * unfold + 0.04 * (t * 1.3 + f as f32).sin();
                let dir = Vec3::new(-ang.sin() * side, -ang.cos(), 0.0);
                let len = (0.45 + 1.3 * u * u) * (0.3 + 0.7 * unfold);
                stroke(root, root + dir * len, wings * (0.9 - 0.3 * u));
            }
        }
        // Feathers drifting down through the frame.
        for k in 0..28u32 {
            let h = |s: u32| hash3(k, s);
            let birth = ANGEL[0].1 + 1.6 * h(1);
            let age = t - birth;
            if age < 0.0 {
                continue;
            }
            let fall = 0.55 + 0.4 * h(2);
            let p = Vec3::new(
                (h(3) - 0.5) * 7.0 + 0.25 * (age * (1.2 + h(4)) + 6.0 * h(5)).sin(),
                2.6 - 0.6 * h(6) - fall * age,
                -0.5 - 2.5 * h(7),
            );
            let amt = ease_out(age / 0.5) * on * (1.0 - 0.6 * leave) * (0.4 + 0.6 * h(8));
            glow(debris, world(p), 0.016 * scale, gold, 0.55 * amt);
        }
        on
    }

    /// World and lyric images, plus the post-process inputs for this frame.
    pub fn render(&self, t: f32, dt: f32, audio: &Audio) -> Frame3d {
        let mut s = self.scene.clone();
        let mut ts = self.text_scene.clone();

        let band = ease_out((t - BAND_IN + 0.1) / 1.2);
        // Before 2.28 the mechanism sits in the dark; the first hit turns the light on.
        let wake = ease_out((t - 2.2) / 0.5);
        let breath = 1.0 - 0.4 * ease_between(t, 59.6, 60.6) * (1.0 - ease_between(t, CHORUS_1 - 0.15, CHORUS_1));
        let drop = 1.0 - 0.45 * ease_between(t, BAND_DROP - 0.3, BAND_DROP + 1.5);
        let band_back = ease_between(t, BAND_BACK - 0.2, BAND_BACK + 0.4);
        let quiet = 1.0
            - 0.55 * ease_between(t, BREAKDOWN, BREAKDOWN + 2.0) * (1.0 - ease_between(t, BAND_RETURN - 0.2, BAND_RETURN + 0.3))
            - 0.4 * ease_between(t, 216.4, 217.0) * (1.0 - ease_between(t, FINAL - 0.1, FINAL))
            - 0.5 * ease_between(t, OUTRO - 0.5, OUTRO + 2.0);
        let world = (0.12 + 0.35 * wake + 0.53 * band) * breath * (drop + 0.45 * band_back).min(1.0) * quiet;
        // Blue: 青い世界 in pre-chorus 1, then the final chorus, closing on 置いて.
        // The light grows as we climb through 高く高く, then opens fully on 青い世界.
        let blue = (0.2 * ease_between(t, PRE_1 + 0.3, BLUE_WORLD - 0.3) + 0.5 * ease_between(t, BLUE_WORLD - 0.4, BLUE_WORLD + 1.2))
            * (1.0 - ease_between(t, 54.2, 60.0))
            ;
        // Bridge: a soft gem glow instead of rays.
        let gem = crate::graphics::mix(t).gem;
        let gem_color = Vec3::new(0.12, 0.62, 0.42).lerp(Vec3::new(0.48, 0.3, 0.8), ease_between(t, BRIDGE_MOON - 1.5, BRIDGE_MOON));
        let (ray_color, rays) = if blue > 0.001 || gem < 0.01 {
            (Vec3::new(0.10, 0.24, 0.78), blue)
        } else {
            (gem_color, 0.45 * gem)
        };
        // The second voice's 好きだった comes with warm light from above.
        let grace = window_f(t, ANGEL[0].1 - 0.2, BRIDGE_MOON + 0.4, 0.7);
        let (ray_color, rays) = (ray_color.lerp(Vec3::new(1.0, 0.86, 0.66), grace), rays.max(0.45 * grace));
        // The air on the planet's limb. Blue is its home (the album's cobalt, brightest under
        // 青い世界), but it takes the colour of the moment: dusk under 鉛のような, the gems in the
        // bridge, gold under the second voice, and it nearly goes out when the floor drops.
        let atmo = {
            let mut a = Vec3::new(0.16, 0.20, 0.46);
            a += Vec3::new(0.03, 0.08, 0.22) * blue;
            a = a.lerp(Vec3::new(0.15, 0.12, 0.30), 0.7 * window_f(t, INTERLUDE, BAND_BACK - 0.5, 2.0));
            a = a.lerp(Vec3::new(0.30, 0.10, 0.18), 0.8 * window_f(t, BAND_BACK - 0.3, CHORUS_2 - 0.6, 0.8));
            a = a.lerp(gem_color * 0.42, gem);
            a = a.lerp(Vec3::new(0.40, 0.30, 0.18), grace);
            a * (1.0 - 0.65 * window_f(t, BREAKDOWN + 0.5, BAND_RETURN, 1.5))
        };

        // Ilios, the sun this town never sees: it hangs eclipsed where 君 looks (behind 青い世界
        // and the last chorus). The moon only moves off in the last chorus, and covers it
        // again on 置いて.
        let sun_amt = 0.0_f32;
        // Dawn on the planet's limb through the last chorus; it closes again on 置いて.
        let eclipse = 0.5 * ease_between(t, FINAL - 0.05, FINAL + 1.6) * (1.0 - ease_between(t, OITE - 1.0, OITE + 3.2));
        // The wish star grows through the breakdown and goes white on the hit; at the end one
        // star hangs where 君 went and answers each chime.
        let wish = (0.35 * ease_between(t, 202.5, 206.0) + 1.4 * ease_in((t - 208.0) / 11.2)) * (1.0 - ease_between(t, FINAL, FINAL + 0.4));
        let chime = 0.7 * cue_pulse(DROPS, t, 1.3) + 0.3 * cue_pulse(KEYS, t, 0.5);
        let chime = if t > 250.0 { chime } else { 0.0 };
        let kimi = (0.75 * ease_between(t, 240.0, 262.0) + chime) * (1.0 - ease_between(t, END - 2.5, END - 0.5));
        // In the run at it, the wish star slides onto the vanishing point: the streaks of the
        // flight leave from it, so it is where we are going.
        let ahead = { let c = self.camera_at(t); (c.look - c.pos).normalize_or_zero() };
        let wish_dir = Vec3::new(0.04, 0.03, -1.0).normalize().lerp(ahead, smooth5(clamp01((t - 206.0) / 4.0))).normalize_or_zero();
        let (star_dir, star_amt) = if t < 228.0 { (wish_dir, wish) } else { (KIMI_DIR, kimi) };
        let iris = window_f(t, BRIDGE - 0.3, BRIDGE_MOON - 1.6, 1.2);
        let voice = window_f(t, BRIDGE_MOON - 1.0, BREAKDOWN, 1.2);
        // The album cover's key: a navy body with cobalt edges.
        let neb_a = Vec3::new(0.15, 0.15, 0.47).lerp(gem_color * 0.6, gem);
        let neb_b = Vec3::new(0.10, 0.30, 0.86).lerp(gem_color, gem);
        // One comet crosses the empty interlude.
        let cx = clamp01((t - 99.0) / 4.5);
        let head = Vec2::new(0.96, 0.10).lerp(Vec2::new(0.22, 0.60), cx);
        let tail = head + Vec2::new(0.74, -0.50).normalize() * 0.24;
        let comet_amt = window_f(t, 99.3, 103.0, 0.5);

        // The lyrics are white; an accent word tints the glow round them instead.
        let (mut gacc, mut gw) = (Vec3::ZERO, 0.0_f32);
        for g in &self.glyphs {
            let (Some(hex), Kind::Main) = (g.accent, g.kind) else { continue };
            let out = self.lines[g.line].out_t;
            if t < g.word_start - 0.15 || t > out + 0.5 {
                continue;
            }
            let w = ease_out((t - g.word_start + 0.15) / 0.3) * (0.4 + 0.6 * (-(t - g.word_start).max(0.0) / 1.5).exp())
                * (1.0 - ease_between(t, out, out + 0.5));
            let c = Vec3::new(((hex >> 16) & 255) as f32, ((hex >> 8) & 255) as f32, (hex & 255) as f32) / 255.0;
            gacc += c * w;
            gw += w;
        }
        let glow_k = clamp01(gw / 2.0);
        let glow_col = Vec3::new(0.46, 0.42, 0.66).lerp(gacc / gw.max(1e-3), glow_k);
        let glow_amt = 0.55 + 0.75 * glow_k + 0.35 * cue_pulse(CHORD, t, 0.3);

        let clock = world_clock(t);
        // Each word inscribed onto the ring in the chorus sends a glow through the circle.
        let spell_since = self
            .lines
            .iter()
            .filter(|l| !l.arrivals.is_empty())
            .flat_map(|l| l.arrivals.iter().skip(1))
            .filter(|a| **a <= t)
            .map(|a| t - a)
            .fold(99.0_f32, f32::min);
        self.graphics.pose(&mut s, t, clock, spell_since, audio);

        // Lyrics.
        let revive = crate::graphics::finale(t) * 0.14;
        let last = self.lines.len() - 1;
        for (li, line) in self.lines.iter().enumerate() {
            let visible = (t >= line.in_t - 0.1 && t < line.out_t + 1.0) || (revive > 0.0 && li > 0) || (li == last && t >= line.in_t);
            ts[line.group].visible = visible;
            if visible && line.spec.exit != 0.0 {
                let up = line.spec.station.rot * Vec3::Y;
                ts[line.group].position =
                    line.spec.station.pos + up * 4.0 * line.spec.exit * ease_in(clamp01((t - line.end + 0.3) / 3.5));
            }
        }
        let mut debris: Vec<Particle> = Vec::new();
        let cam_now = self.camera_at(t);
        let cam_fwd_now = (cam_now.look - cam_now.pos).normalize_or_zero();
        // Where each phrase of 鉛のような… sits in the laid-out line, so each can be brought to the centre.
        let mut lead_rows = [0.0_f32; 3];
        for (ri, row_y) in lead_rows.iter_mut().enumerate() {
            let ys: Vec<f32> = self.glyphs.iter().filter(|g| g.line == 17 && lead_row(g.word_start) == ri).map(|g| g.base.y).collect();
            *row_y = ys.iter().sum::<f32>() / ys.len().max(1) as f32;
        }
        for (g, base, t0, t1, size, parts) in &self.staffs {
            let op = window_f(t, *t0, *t1, 0.3) * 0.3;
            ts[*g].visible = op > 0.003;
            if ts[*g].visible {
                let shift = -2.6 * (t - t0);
                ts[*g].position = *base + Vec3::new(shift, 0.0, 0.0);
                // The staff is a string the melody plays: two waves run along it, louder with the song.
                let amp = size * (0.22 + 0.5 * audio.bins.get(9).copied().unwrap_or(0.3).min(1.0));
                let wave = |x: f32| amp * (0.65 * (x * 0.42 - t * 5.2).sin() + 0.35 * (x * 0.17 + t * 2.9).sin());
                for (n, x, y) in parts {
                    let wx = x + shift;
                    ts[*n].position.y = y + wave(wx);
                    if (ts[*n].scale.x - 1.0).abs() < 0.01 && (y.abs() / (0.36 * size)).fract() < 0.01 {
                        ts[*n].rotation = Quat::from_rotation_z(((wave(wx + 0.3) - wave(wx - 0.3)) / 0.6).atan());
                    }
                }
                for d in ts.descendants(*g) {
                    if let Some(m) = ts[d].material_mut() {
                        m.opacity = op;
                    }
                }
            }
        }
        // 向こうへと向かうんだ recedes ahead of us.
        {
            let l = &self.lines[13];
            let away = clamp01((t - 93.0) / 10.0);
            ts[l.group].position = l.spec.station.pos - (l.spec.station.rot * Vec3::Z) * 70.0 * away * away;
        }
        for g in &self.glyphs {
            let line = &self.lines[g.line];
            if !ts[line.group].visible {
                continue;
            }
            if let Some((k, _)) = g.cluster {
                // YOASOBI-style cluster: newer words push this one aside, smaller and dimmer;
                // the whole cluster collapses at the end. Ghost copies trail it in time.
                let lag = match g.kind {
                    Kind::Ghost(n) => 0.035 * n as f32,
                    _ => 0.0,
                };
                let tt = t - lag;
                let n: f32 = line.arrivals.iter().skip(k + 1).map(|a| expo_out((tt - a + 0.06) / 0.55)).sum();
                let since = tt - line.arrivals[k] + 0.12;
                let arrive = expo_out(since / 0.6);
                let flip = expo_out((since - 0.08) / 0.7); // the flip settles after the position
                let collapse = expo_in((tt - line.end - 0.2) / 0.45);
                // Casting a spell: once a newer word lands, this one flies up and is inscribed
                // onto the magic circle, written along the ring, and keeps sliding round it as
                // more words follow. When the line ends the spell flares and fades.
                let into_ring = expo_out(n.min(1.0));
                let st = line.spec.station;
                let a_s = st.pos.y.atan2(st.pos.x);
                let ring_r = crate::graphics::DIAL_R * 1.25 * 0.905;
                let sc = 0.42;
                let phi = a_s + deg(9.0) + deg(13.0) * (n - 1.0).max(0.0);
                let phi_g = phi - (g.base.x * sc) / ring_r;
                let rot_w = Quat::from_rotation_z(phi_g - PI / 2.0);
                let pos_w = Vec3::new(0.0, 0.0, 1.45) + Vec3::new(phi_g.cos(), phi_g.sin(), 0.0) * ring_r + rot_w * Vec3::new(0.0, g.base.y * sc, 0.0);
                let inv = st.rot.inverse();
                let ring_local = inv * (pos_w - st.pos);
                let ring_rot = inv * rot_w;
                let centre_local = Vec3::new(0.0, 0.15, 3.2 * (1.0 - arrive)) + g.base * (1.0 + 0.04 * clamp01(since / 3.0));
                // Fly up in an arc toward the camera on the way to the ring.
                let lift = Vec3::new(0.0, 0.0, 1.6 * (into_ring * PI).sin());
                let flare = (-((tt - line.end - 0.3).max(0.0)) / 0.35).exp() * if tt > line.end + 0.3 { 1.0 } else { 0.0 };
                let mut opacity = expo_out(since / 0.25) * (1.0 - collapse) * (1.0 - 0.75 * (1.0 - flip))
                    * (1.0 - 0.2 * into_ring) * clamp01(6.0 - n);
                if let Kind::Ghost(_) = g.kind {
                    opacity = 0.0;
                }
                let node = &mut ts[g.node];
                node.visible = since > 0.0 && opacity > 0.002;
                if !node.visible {
                    continue;
                }
                node.position = centre_local.lerp(ring_local, into_ring) + lift;
                node.scale *= 1.0 + (sc - 1.0) * into_ring + 0.25 * flare;
                let side = if (k % 2) == 0 { 1.0 } else { -1.0 };
                let upright = Quat::from_rotation_y(side * 1.1 * (1.0 - flip)) * Quat::from_rotation_z(g.tilt);
                node.rotation = upright.slerp(ring_rot, into_ring);
                if let Some(m) = node.material_mut() {
                    m.opacity = opacity;
                    m.color = g.color * if t < g.word_end + 0.15 { 1.0 } else { 0.86 };
                }
                if let Some(p) = g.plate {
                    pose_echo(&mut ts, p, g, t, opacity);
                }
                continue;
            }

            let assemble = g.scatter != Vec3::ZERO;
            let mut glyph_boost = 1.0_f32;
            let since = t - g.appear;
            if let Kind::Ghost(n) = g.kind {
                // Breath: each copy lifts off the held letter, drifts up and away, swells and
                // thins out. The song's spectrum nudges it as it goes.
                let life = 2.4;
                let node = &mut ts[g.node];
                node.visible = since > 0.0 && since < life;
                if !node.visible {
                    continue;
                }
                let w = g.width;
                let bin = audio.bins.get((n as usize * 5 + 3) % 32).copied().unwrap_or(0.4);
                let side = if n % 2 == 0 { 1.0 } else { -1.0 };
                node.position = g.base
                    + Vec3::new(w * (0.16 * since + 0.10 * side * (since * 1.9 + n as f32).sin()), w * (0.5 * since + 0.22 * (bin - 0.4) * since), -0.7 * since);
                node.scale *= 1.0 + 0.22 * since;
                let o = ease_out(since / 0.2) * (1.0 - since / life).powi(2) * 0.6 * (1.0 - ease_between(t, line.out_t, line.out_t + 0.4));
                if let Some(m) = node.material_mut() {
                    m.opacity = o;
                }
                continue;
            }
            // Properties settle one after another: position, then rotation, then scale.
            let pos = expo_out(since / if assemble { 1.0 } else { 0.6 });
            let rot = expo_out((since - 0.09) / 0.7);
            let scl = expo_out((since - 0.04) / 0.75);
            let fade_in = expo_out(since / 0.22);
            // Exit: accelerate away (expo-in) instead of a plain fade.
            let q = if g.line == last { ease_between(t, 250.5, 253.4) } else { expo_in((t - line.out_t) / 0.5) };
            let mut opacity = (fade_in * (1.0 - q)).max(revive * pos);
            if let Kind::Echo(n) = g.kind {
                opacity *= 0.5_f32.powi(n as i32) * 0.55;
            }
            let node = &mut ts[g.node];
            node.visible = since > 0.0 && opacity > 0.002;
            if !node.visible {
                continue;
            }
            // After landing the line never fully stops: a slow creep toward the camera.
            let life = clamp01((t - line.start) / (line.out_t - line.start).max(1.0));
            let drift = Vec3::new(0.0, 0.0, 0.35 * life) + Vec3::new(0.0, 0.0, -1.4 * q);
            let (offset, grow, flip) = if g.slam {
                // Heavy words: a light spring on depth and scale.
                let sp = spring(since);
                (Vec3::new(0.0, 0.0, 1.2 * (1.0 - sp)), 1.0 + 0.5 * (1.0 - sp), 0.0)
            } else if assemble {
                (g.scatter * (1.0 - pos), 1.0, 0.0)
            } else {
                match line.spec.entrance {
                    Entrance::Rise => (Vec3::new(0.0, -0.8 * (1.0 - pos), 0.0), 1.0, 0.0),
                    Entrance::Fall => (Vec3::new(0.0, 1.4 * (1.0 - ease_in_out_heavy(since / 0.7)), 0.0), 1.0, 0.0),
                    Entrance::Descend => (Vec3::new(0.0, 2.6 * (1.0 - expo_out(since / 0.9)), 0.35 * (1.0 - pos)), 1.0, 0.0),
                    _ => (Vec3::new(0.0, -0.1 * (1.0 - pos), 1.0 * (1.0 - pos)), 1.0 + 0.14 * (1.0 - scl), 0.55 * (1.0 - rot)),
                }
            };
            node.position = g.base + offset + drift;
            if g.line < 38 {
                // A held note: the letter-spacing opens for as long as the voice holds.
                let open = HOLDS.iter().filter(|h| h.0 == g.line).map(|h| ease_between(t, h.3, h.4)).fold(0.0_f32, f32::max);
                node.position.x += g.base.x * 0.28 * open;
            }
            if g.line == 17 {
                // 鉛のようなこの足を睨んで君を待つ: one phrase at a time, alone in the frame. Each
                // sinks slowly while it is sung, heavy as lead, and drops away when the next
                // begins. 君を待つ does not sink: it waits, and its letters drift apart.
                let ri = lead_row(g.word_start);
                let (a, b) = [(126.77, 129.0), (129.0, 132.46), (132.46, 134.6)][ri];
                let size = line.spec.size;
                let gone = (t - b + 0.3).max(0.0);
                node.position.y = g.base.y - lead_rows[ri] + drift.y;
                if ri < 2 {
                    node.position.y += 0.45 * size - 0.24 * size * (t - a).max(0.0) - 5.0 * size * gone * gone;
                    opacity *= 1.0 - clamp01(gone / 0.28);
                } else {
                    node.position.x += g.base.x * 0.4 * ease_between(t, 132.46, 134.4);
                }
            }
            if let Kind::Echo(n) = g.kind {
                // The echoes spread back from the word after it lands.
                node.position.z -= 0.9 * n as f32 * ease_out(since / 1.6);
            }
            // The last だ of 抱えて向かうんだ stays: the camera plunges through it.
            let last_da = g.line == 12 && g.word_start > 87.7;
            if ![6, 13].contains(&g.line) && !last_da {
                // Words dissolve as the camera reaches them, so it flies through, not into them.
                let st = line.spec.station;
                let ahead = (cam_now.pos - st.world(node.position)).dot(st.rot * Vec3::Z);
                opacity *= clamp01((ahead - 1.6) / 2.2);
            }
            if line.spec.flow == Flow::PunchBack {
                // ...and each phrase gives way to the one that arrives in front of it.
                if let Some((next, _)) = line.words.iter().find(|(wt, c)| *wt > g.word_start && (c.z - g.base.z).abs() > 0.5) {
                    opacity *= 1.0 - ease_between(t, next - 0.55, next - 0.15);
                }
            }
            if matches!(line.spec.flow, Flow::Punch | Flow::PunchBack) {
            }
            // 笑顔で言う: each letter is pulled into the singularity behind the line — it
            // stretches toward it, spirals and is swallowed.
            if g.line == 6 {
                let pull = expo_in(clamp01((t - WORMHOLE - 0.05 - hash3(g.node.0, 3) * 0.4) / 1.25));
                let centre = Vec3::new(0.0, 0.35, -1.8);
                let to = centre - g.base;
                let ang = 2.8 * pull * pull;
                let swirl = Quat::from_rotation_z(ang) * (g.base - centre) * (1.0 - pull);
                node.position = centre + swirl + Vec3::new(0.0, 0.0, 0.0) * to.length();
                node.scale *= Vec3::new(1.0 - 0.75 * pull, 1.0 + 1.8 * pull * (1.0 - pull), 1.0);
                node.rotation = Quat::from_rotation_z(ang + 1.2 * pull) * node.rotation;
                opacity *= 1.0 - pull * pull * pull;
            }
            if last_da {
                node.scale *= 1.0 + 1.6 * clamp01((t - 87.85) / 2.25).powi(2);
            }
            // 向こうへと向かうんだ: the words do what they say. One by one each letter is drawn out
            // flat into a dash of light, and the dash leaves for the far side, where the two
            // lights of the interlude are about to go.
            if g.line == 13 {
                let death = 96.0 + hash3(g.node.0, 21) * 4.5;
                let squash = ease_between(t, death, death + 0.45);
                let fade = 1.0 - ease_between(t, 104.0, 110.0);
                let centre = Vec3::new(g.width * 0.5, g.width * 0.4, 0.0);
                let sc = Vec3::new(1.0 + 0.9 * squash, 1.0 - 0.96 * squash, 1.0);
                node.position += centre * (Vec3::ONE - sc);
                node.scale *= sc;
                glyph_boost = 1.0 + 2.0 * squash;
                opacity *= (1.0 - ease_between(t, death + 0.35, death + 0.6)) * fade;
                if t > death + 0.3 {
                    let st = line.spec.station;
                    let h = |n: u32| hash3(g.node.0, n) - 0.5;
                    let dir = (st.rot * Vec3::new(0.18 * h(23), 0.2 + 0.12 * h(24), -1.0)).normalize();
                    let at = |tt: f32| {
                        let away = clamp01((tt - 93.0) / 10.0);
                        let fly = (tt - death - 0.4).max(0.0);
                        st.pos - (st.rot * Vec3::Z) * 70.0 * away * away + st.rot * (g.base + centre) + dir * 26.0 * fly.powf(2.4)
                    };
                    let path: Vec<Vec3> = (0..40u32).map(|k| at(t - k as f32 * 0.03)).collect();
                    let far = (path[0] - cam_now.pos).length();
                    let born = ease_between(t, death + 0.3, death + 0.6);
                    let out = 1.0 - ease_between(t, death + 2.2, death + 4.2);
                    light_trail(&mut debris, &path, 0.006 * (8.0 + far * 0.5), Rgb::hex(0xcfc4ff), born * out * fade);
                }
            }
            if (26..=31).contains(&g.line) {
                // Afloat: the letters ride a slow swell.
                node.position.y += 0.08 * line.spec.size * (t * 1.6 + g.base.x * 1.2).sin();
            }
            if g.line == 0 && t > BAND_IN {
                node.scale *= 1.0 + 0.05 * audio.low;
            }
            if g.line == 16 {
                node.position.x += g.base.x.signum() * 0.9 * ease_between(t, 120.6, 123.8);
            }
            node.scale *= grow * (1.0 + 0.03 * life);
            let mut glyph_color = g.color;
            if line.spec.entrance == Entrance::Descend {
                let d = expo_out(since / 0.9);
                node.scale *= Vec3::new(0.7 + 0.3 * d, 1.0 + 2.2 * (1.0 - d), 1.0);
                glyph_color = Rgb::hex(0xffffff).lerp(g.color, expo_out((since - 0.2) / 1.2));
            }
            let spin = if assemble { (1.0 - rot) * 2.4 * (g.scatter.x.signum()) } else { 0.0 };
            node.rotation = Quat::from_rotation_x(-flip) * Quat::from_rotation_z(g.tilt * (0.3 + 0.7 * rot) + spin);
            let lit = if t >= g.word_start && t < g.word_end + 0.15 {
                1.0
            } else if t >= g.word_end {
                0.84
            } else {
                0.92
            };
            if let Some(m) = node.material_mut() {
                m.opacity = opacity;
                m.color = glyph_color * (lit * glyph_boost);
            }
            if let Some(p) = g.plate {
                pose_echo(&mut ts, p, g, t, opacity);
            }
        }

        // 二人: two small lights circling each other, from the interlude through verse 2. On
        // 違う夢 one of them drifts off.
        let lights_on = window_f(t, 96.5, 125.0, 1.5);
        if lights_on > 0.003 {
            let follow = ease_between(t, 110.5, 113.5);
            let split = ease_between(t, 119.8, 124.5);
            let (mut blue_path, mut pink_path) = (Vec::new(), Vec::new());
            let leave = |up: Vec3| (up * 2.6 + Vec3::new(2.4, 0.0, -1.5)) * split;
            for k in 0..56u32 {
                let tt = t - k as f32 * 0.022;
                let mut c = self.lights_path(tt);
                let mut up = Vec3::Y;
                if follow > 0.0 {
                    let p = self.camera_at(tt);
                    up = p.up.normalize_or_zero();
                    c = c.lerp(p.look + up * 1.9, follow);
                }
                let th = tt * 1.1;
                let orbit = Vec3::new(th.cos() * 0.55, th.sin() * 0.35, th.sin() * 0.55);
                blue_path.push(c + orbit + leave(up));
                pink_path.push(c - orbit);
            }
            let breathe = 1.0 + 0.1 * (t * 5.3).sin() + 0.06 * (t * 8.9 + 1.0).sin();
            light_trail(&mut debris, &blue_path, 0.115 * breathe, Rgb::hex(0x7fb0ff), lights_on * (1.0 - split));
            light_trail(&mut debris, &pink_path, 0.115 * (2.0 - breathe), Rgb::hex(0xff8f9c), lights_on);
        }
        let kimi_on = window_f(t, self.lines[37].start + 0.1, 235.3, 0.35);
        if kimi_on > 0.003 {
            let path: Vec<Vec3> = (0..50u32).map(|k| self.kimi_light(t - k as f32 * 0.02)).collect();
            // It keeps its size on screen as it flies away, so it stays a light and not a dot.
            let far = (path[0] - cam_now.pos).length();
            light_trail(&mut debris, &path, 0.018 * (6.0 + far * 0.5), Rgb::hex(0x7fb0ff), kimi_on);
        }
        let mut asc = self.angel_scene.clone();
        let angel_amt = self.pose_angel(&mut asc, &mut debris, t, &cam_now);
        if !debris.is_empty() {
            s.add(Node::particles(debris, Blending::Additive));
        }
        let items = self
            .dust
            .iter()
            .map(|(p, size, speed, op)| {
                // The motes fill a box that travels with the camera, so we are always flying through them.
                let raw = Vec3::new(p.x + 0.2 * (clock * 0.3 + p.z).sin(), p.y + clock * speed, p.z) - cam_now.pos + cam_fwd_now * -warp_travel(t);
                let wrap = |x: f32| (x + 20.0).rem_euclid(40.0) - 20.0;
                Particle::new(cam_now.pos + Vec3::new(wrap(raw.x), wrap(raw.y), wrap(raw.z)), *size * 0.7, Rgb::hex(LILAC) * (0.35 * world + 0.06))
                    .opacity(*op * 0.6)
            })
            .collect();
        s.add(Node::particles(items, Blending::Additive));


        // Motion for the post blur: where the look target was on screen half a frame ago.
        let pose = cam_now;
        let camera = pose.camera();
        let prev_pose = self.camera_at(t - dt * 0.5);
        let prev = prev_pose.camera();
        let (w, h) = (1920.0, 1080.0);
        let dist = (pose.look - pose.pos).length().max(0.5);
        let pan = prev.project(pose.look, w, h).map(|p| Vec2::new(w * 0.5 - p.x, h * 0.5 - p.y)).unwrap_or(Vec2::ZERO);
        let zoom = (pose.pos - prev_pose.pos).dot(camera.forward()) / dist;
        let (u1, u0, f) = (camera.up_vector(), prev.up_vector(), camera.forward());
        if std::env::var_os("ILIOS_CAM_DEBUG").is_some() {
            eprintln!("CAM t={:.2} pos={:?} fwd={:?} up={:?} fov={:.1}", t, pose.pos, f, u1, pose.fov);
        }
        let spin = wrap_pi(u0.cross(u1).dot(f).atan2(u0.dot(u1)));

        Frame3d {
            world: self.renderer.render(&s, &camera),
            text: self.text_renderer.render(&ts, &camera),
            pan: pan.clamp_length_max(260.0),
            zoom: zoom.clamp(-0.4, 0.4),
            spin: spin.clamp(-0.2, 0.2),
            hit: impact(t, 0.2).min(1.5),
            blue: rays,
            ray_color,
            red_flash: 0.0
                + 0.3 * ease_out((t - 153.21) / 0.15) * (1.0 - ease_between(t, 153.5, SABISHIKU_2 + 1.5))
                + 0.25 * ease_between(t, OITE - 0.3, OITE + 0.4) * (1.0 - ease_between(t, 242.0, 246.0)),
            world_light: world,
            invert: if BURSTS.iter().any(|b| (b + 0.067..b + 0.134).contains(&t)) || INVERTS.iter().any(|b| (*b..b + 0.067).contains(&t)) {
                1.0
            } else {
                0.0
            },
            red_frame: if BURSTS.iter().any(|b| (*b..b + 0.067).contains(&t)) { 1.0 } else { 0.0 },
            glitch: decay(GLITCHES, t, 0.1).min(1.2).max(0.55 * wormhole(t)) + 0.25 * window_f(t, KURUSHIKU, 74.3, 0.1),
            speed: warp(t),
            twirl: wormhole(t) * (1.0 - ease_between(t, CHORUS_1 - 0.04, CHORUS_1)),
            // Not `camera.right()`: that ignores roll, which sheared the sky on rolled shots.
            cam_right: camera.forward().cross(camera.up_vector()).normalize(),
            cam_up: camera.up_vector(),
            cam_fwd: camera.forward(),
            tan_half_fov: (deg(pose.fov) * 0.5).tan(),
            low: audio.low,
            // 苦しく寂しく: into a broken black-and-white TV; on the last く the camera drags out
            // and the set switches off to a line.
            tv: window_f(t, KURUSHIKU - 0.05, 75.3, 0.6) + window_f(t, 153.16, 157.9, 0.6),
            // 届くはずのない…: "The World". An inverted dome sweeps out and the world goes grey
            // and still; it sweeps back in when time restarts.
            // The dome sweeps past the corners (2.04 half-heights away) so it fills the frame.
            dome: TIME_STOPS
                .iter()
                .map(|&(a, b)| {
                    if t >= a && t < a + 0.7 {
                        2.4 * expo_out((t - a) / 0.6)
                    } else if t > b - 0.4 && t < b + 0.05 {
                        2.4 * (1.0 - expo_in((t - b + 0.4) / 0.42))
                    } else {
                        0.0
                    }
                })
                .fold(0.0, f32::max),
            stop: TIME_STOPS.iter().map(|&(a, b)| window_f(t, a + 0.35, b - 0.1, 0.25)).fold(0.0, f32::max),
            flash: (-((t - CHORUS_1).max(0.0)) / 0.28).exp() * if t >= CHORUS_1 - 0.02 { 1.0 } else { 0.0 } * 0.9
                + TIME_STOPS.iter().map(|&(_, b)| 0.45 * (-((t - b).max(0.0)) / 0.25).exp() * if t >= b { 1.0 } else { 0.0 }).sum::<f32>()
                + 1.0 * (-((t - FINAL).max(0.0)) / 0.5).exp() * if t >= FINAL { 1.0 } else { 0.0 }
                + 0.6 * (-((t - 90.1).max(0.0)) / 0.3).exp() * if t >= 90.1 { 1.0 } else { 0.0 },
            crt: 0.0,
            sun_dir: Vec3::new(0.0, -0.262, -0.965),
            sun_amt,
            eclipse,
            neb_a,
            neb_b,
            comet: [tail.x, tail.y, head.x, head.y],
            comet_amt,
            // A pool of shade behind the title, so the magic circle's lines dim where they pass
            // the letters and the title stands in front of it.
            shade: 0.85 * ease_between(t, BAND_IN + 0.1, BAND_IN + 1.2) * (1.0 - ease_between(t, 27.0, 28.6)),
            glow_col,
            glow_amt,
            star_dir,
            star_amt,
            iris,
            voice,
            beat: beat_pulse(t),
            toll: toll(t),
            swell: cue_swell(WHEEZE, t, 0.12, 0.75).min(1.6),
            snare: cue_pulse(SNARE, t, 0.14).min(1.2),
            staff: ease_between(t, 253.4, 254.3) * (1.0 - ease_between(t, 268.5, 271.8)),
            picks: {
                // Each pick lands on one of the five strings. Its measured note is folded onto the
                // staff (two diatonic steps to a string), so the line keeps the melody's shape and
                // every note sits exactly on a string.
                let n = GUITAR.iter().filter(|g| **g <= t).count();
                let pick = |back: usize| {
                    if n <= back {
                        return Vec3::ZERO;
                    }
                    let k = n - 1 - back;
                    Vec3::new((staff_step(GUITAR_NOTES[k]).rem_euclid(10.0) / 2.0).floor(), t - GUITAR[k], (-(t - GUITAR[k]) / 0.9).exp())
                };
                [pick(0), pick(1)]
            },
            drop: DROPS.iter().enumerate().rev().find(|(_, d)| **d <= t).map_or(Vec3::ZERO, |(k, d)| Vec3::new(0.3 + 0.4 * hash3(k as u32, 31), t - d, (-(t - d) / 1.5).exp())),
            keys: {
                // Chords strike wide and bright, single keys softer; each lands on its own key.
                let mut struck: Vec<(f32, f32)> = CHORD.iter().map(|c| (*c, 1.0)).chain(KEYS.iter().map(|c| (*c, 0.55))).filter(|(c, _)| *c <= t && t - *c < 5.0).collect();
                struck.sort_by(|a, b| b.0.total_cmp(&a.0));
                let key = |n: usize| struck.get(n).map_or(Vec3::ZERO, |&(c, k)| Vec3::new(0.08 + 0.84 * (c * 7.31).fract(), t - c, k * (-(t - c) / 0.7).exp()));
                [key(0), key(1)]
            },
            angel: self.angel_renderer.render(&asc, &camera),
            angel_amt,
            atmo,
            echo: VOCAL_ECHO.iter().rev().find(|c| **c <= t).map_or((9.0, 0.0), |c| (t - c, (-(t - c) / 0.55).exp())),
        }
    }
}
