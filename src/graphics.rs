//! Flat graphic elements laid out in space: constellations, magic circles, hairline grids,
//! rings and altitude ticks. No solid objects; the depth comes from the camera.
use crate::timing::*;
use fframes::three_d::*;

pub const LILAC: u32 = 0x9d93d6;
pub const RED_HOT: u32 = 0xff5a6e;
const STAR: u32 = 0xf2ecff;

/// The chorus dial (the lyric stations sit on it).
pub const DIAL_R: f32 = 9.6;
/// When the credits start being written onto the title circle.
pub const CREDITS_AT: f32 = BAND_IN + 4.0 * BEAT + 0.4;
/// Loudness of the song at this frame, from fframes' FFT (`Frame::visualize_audio_frame`),
/// each 0..~1.
#[derive(Clone, Debug, Default)]
pub struct Audio {
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    /// Log-spaced bands, low to high.
    pub bins: Vec<f32>,
}

fn line_mat(color: u32) -> Material {
    Material::unlit(Rgb::hex(color)).transparent().opacity(0.0)
}

/// A hairline from `a` to `b` (a thin box), thickness `w`.
fn line(a: Vec3, b: Vec3, w: f32, mat: Material) -> Node {
    let d = b - a;
    let len = d.length().max(1e-4);
    Node::mesh(Geometry::cuboid(w, len, w), mat)
        .with_position((a + b) * 0.5)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d / len))
}

fn hash(i: u32, s: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E3779B1) ^ s.wrapping_mul(0x85EBCA77);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B3C6D);
    x ^= x >> 12;
    (x & 0xffff) as f32 / 65535.0
}

struct Star {
    node: NodeId,
    at: f32,
    size: f32,
    red: bool,
}

struct Link {
    node: NodeId,
    len: f32,
    at: f32,
    thick: f32,
}

/// Real constellations: (name, stars as (right ascension in hours, declination in
/// degrees, magnitude), stick-figure edges in drawing order). Positions are the standard
/// catalogue values of the bright stars, rounded.
pub const SKY: &[(&str, &[(f32, f32, f32)], &[(usize, usize)])] = &[
    ("CASSIOPEIA", &[(0.153, 59.15, 2.3), (0.675, 56.54, 2.2), (0.945, 60.72, 2.4), (1.430, 60.24, 2.7), (1.907, 63.67, 3.4)],
        &[(0, 1), (1, 2), (2, 3), (3, 4)]),
    ("ORION", &[(5.919, 7.41, 0.5), (5.418, 6.35, 1.6), (5.679, -1.94, 1.8), (5.604, -1.20, 1.7), (5.533, -0.30, 2.2),
        (5.796, -9.67, 2.1), (5.242, -8.20, 0.1), (5.585, 9.93, 3.4)],
        &[(7, 0), (7, 1), (0, 2), (1, 4), (2, 3), (3, 4), (2, 5), (4, 6)]),
    ("URSA MAJOR", &[(11.062, 61.75, 1.8), (11.031, 56.38, 2.4), (11.897, 53.69, 2.4), (12.257, 57.03, 3.3), (12.900, 55.96, 1.8),
        (13.399, 54.93, 2.2), (13.792, 49.31, 1.9)],
        &[(6, 5), (5, 4), (4, 3), (3, 0), (0, 1), (1, 2), (2, 3)]),
    ("LYRA", &[(18.616, 38.78, 0.0), (18.746, 37.61, 4.3), (18.908, 36.90, 4.3), (18.982, 32.69, 3.3), (18.835, 33.36, 3.5)],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 1)]),
    ("CYGNUS", &[(20.690, 45.28, 1.3), (20.370, 40.26, 2.2), (20.770, 33.97, 2.5), (19.750, 45.13, 2.9), (19.512, 27.96, 3.1)],
        &[(0, 1), (1, 4), (3, 1), (1, 2)]),
    ("LEO", &[(10.139, 11.97, 1.4), (10.122, 16.76, 3.5), (10.333, 19.84, 2.0), (10.278, 23.42, 3.4), (9.879, 26.01, 3.9),
        (9.764, 23.77, 3.0), (11.235, 20.52, 2.6), (11.237, 15.43, 3.3), (11.818, 14.57, 2.1)],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (2, 6), (6, 8), (8, 7), (7, 0)]),
    ("GEMINI", &[(7.577, 31.89, 1.6), (7.755, 28.03, 1.1), (6.732, 25.13, 3.0), (6.383, 22.51, 2.9), (6.629, 16.40, 1.9),
        (7.335, 21.98, 3.5), (7.068, 20.57, 3.8)],
        &[(0, 1), (0, 2), (2, 3), (1, 5), (5, 6), (6, 4)]),
    ("SCORPIUS", &[(16.490, -26.43, 1.0), (16.091, -19.81, 2.6), (16.006, -22.62, 2.3), (15.981, -26.11, 2.9), (16.353, -25.59, 2.9),
        (16.598, -28.22, 2.8), (16.836, -34.29, 2.3), (16.864, -38.05, 3.0), (16.910, -42.36, 3.6), (17.203, -43.24, 3.3),
        (17.622, -43.00, 1.9), (17.793, -40.13, 3.0), (17.708, -39.03, 2.4), (17.560, -37.10, 1.6)],
        &[(1, 2), (2, 3), (2, 4), (4, 0), (0, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11), (11, 12), (12, 13)]),
];

/// The intro flight: the camera travels forward through the sky from far out and arrives
/// in front of the title on the band's first downbeat (0:15.79).
pub fn flight(t: f32) -> Vec3 {
    let s = clamp01(t / BAND_IN);
    let settle = 1.0 - s.powi(4);
    Vec3::new(
        4.2 * (t * 0.31).sin() * settle,
        1.0 + 2.4 * (t * 0.23 + 0.5).sin() * settle,
        150.0 - 119.0 * (0.82 * s + 0.18 * s.powi(4)),
    )
}

/// Where each constellation hangs and when its edges draw.
pub fn constellation_plan() -> Vec<(usize, Vec3, f32, Vec<f32>)> {
    let notes: Vec<f32> = PIANO.iter().flat_map(|p| p.iter().copied()).collect();
    let mut plan = Vec::new();
    let mut n = 0;
    let sides = [(-6.5, 3.0), (7.0, -1.5), (-6.5, -2.5), (6.5, 3.5), (-7.0, 1.0), (6.0, -3.0)];
    for c in 0..6 {
        let edges = SKY[c].2.len();
        let times: Vec<f32> = (0..edges).map(|k| notes.get(n + k).copied().unwrap_or(BAND_IN + k as f32 * 0.45)).collect();
        n += edges;
        let mid = times[times.len() / 2];
        let (sx, sy) = sides[c];
        let centre = flight(mid + 1.0) + Vec3::new(sx * 0.7, sy * 0.7, -13.0);
        plan.push((c, centre, 0.42, times));
    }
    // 二人: Gemini, the twins, drawn word by word in verse 2.
    plan.push((6, Vec3::new(4.0, 14.0, -322.0), 0.42, vec![112.39, 113.25, 114.46, 115.42, 116.58, 117.76]));
    // The wish star: Scorpius with red Antares, drawn slowly in the breakdown.
    plan.push((7, Vec3::new(7.0, 5.0, -690.0), 0.36, (0..13).map(|k| 202.9 + k as f32 * 0.5).collect()));
    plan
}

pub struct Graphics {
    stars: Vec<Star>,
    links: Vec<Link>,
    gem: NodeId,
    /// Magic circle round the title (0:15.8) and the big one the choruses happen on.
    title_circle: MagicCircle,
    /// How bright the title's circle is against the video's own mix (1); the thumbnail lifts it.
    pub title_circle_gain: f32,
    chorus_circle: MagicCircle,
    /// 高く高く: altitude ticks streaming past as the camera climbs.
    ruler: Vec<(NodeId, f32)>,
    rings: Vec<NodeId>,
    ring_events: Vec<(f32, f32)>,
}

/// One separately lit part of a magic circle.
struct Piece {
    node: NodeId,
    /// Order from the centre; the travelling glow reaches higher layers later.
    layer: f32,
    /// Seconds after the circle starts drawing that this part is drawn.
    delay: f32,
    gain: f32,
    color: u32,
    /// Angle on its ring for the parts a light runs round; NaN for the rest.
    ang: f32,
}

/// The sun's own circle (イリオス is ἥλιος, the sun), and it "controls time". From the centre: a
/// rayed sun, a {12/5} star lattice, a quiet band with the credits written along its lower arc,
/// five rune circles with arrows inside a ring of spikes, the clock numerals (they tick once
/// a beat), a band of time-runes, a graduated scale, the outer ring with a light running round
/// it, the eight moon phases, and a great eight-pointed star that is larger than the frame.
/// Every part is drawn by a sweep, layers turn at their own speeds and sit at their own depths,
/// and the whole disc sways a little, so the depths slide against each other.
struct MagicCircle {
    root: NodeId,
    pieces: Vec<Piece>,
    /// (layer group, radians per second of `clock`).
    spin: Vec<(NodeId, f32)>,
    numerals: NodeId,
}

const WARM: u32 = 0xffe6cf;

/// A flat band between radii `r0` and `r1` from angle `a0` to `a1`, as triangles.
fn band(r0: f32, r1: f32, a0: f32, a1: f32) -> Vec<Vec3> {
    let n = ((a1 - a0).abs() / deg(2.0)).ceil().max(1.0) as usize;
    let p = |r: f32, k: usize| {
        let a = a0 + (a1 - a0) * k as f32 / n as f32;
        Vec3::new(a.cos() * r, a.sin() * r, 0.0)
    };
    (0..n).flat_map(|k| [p(r0, k), p(r1, k), p(r1, k + 1), p(r0, k), p(r1, k + 1), p(r0, k + 1)]).collect()
}

/// A flat hairline from `a` to `b`, as triangles.
fn stroke(a: Vec3, b: Vec3, w: f32) -> Vec<Vec3> {
    let d = (b - a).normalize_or_zero();
    let n = Vec3::new(-d.y, d.x, 0.0) * w * 0.5;
    vec![a - n, a + n, b + n, a - n, b + n, b - n]
}

fn polar(a: f32, r: f32) -> Vec3 {
    Vec3::new(a.cos() * r, a.sin() * r, 0.0)
}

/// A filled dot.
fn dot(c: Vec3, r: f32) -> Vec<Vec3> {
    band(0.0, r, 0.0, std::f32::consts::TAU).into_iter().map(|p| p + c).collect()
}

/// A filled rhombus, its long axis along `a`.
fn diamond(c: Vec3, a: f32, long: f32, short: f32) -> Vec<Vec3> {
    let (u, v) = (polar(a, long), polar(a + deg(90.0), short));
    vec![c + u, c + v, c - u, c + u, c - u, c - v]
}

struct Draft<'a> {
    scene: &'a mut Scene,
    pieces: Vec<Piece>,
}

impl Draft<'_> {
    fn geo(&mut self, parent: NodeId, geo: Geometry, layer: f32, delay: f32, gain: f32, color: u32) -> NodeId {
        let node = self.scene.add_to(parent, Node::mesh(geo, Material::glow(Rgb::hex(color), 0.0).double_sided()));
        self.pieces.push(Piece { node, layer, delay, gain, color, ang: f32::NAN });
        node
    }

    fn tris(&mut self, parent: NodeId, tris: Vec<Vec3>, layer: f32, delay: f32, gain: f32, color: u32) -> NodeId {
        self.geo(parent, Geometry::from_triangles(&tris), layer, delay, gain, color)
    }

    /// A ring drawn by a sweep that starts at angle `from` at `delay` and takes `sweep` seconds.
    /// `orbit`: a light runs round it afterwards.
    #[allow(clippy::too_many_arguments)]
    fn ring(&mut self, parent: NodeId, r: f32, w: f32, layer: f32, delay: f32, sweep: f32, from: f32, gain: f32, orbit: bool) {
        use std::f32::consts::TAU;
        let n = 36;
        for k in 0..n {
            let (a0, a1) = (from + TAU * k as f32 / n as f32, from + TAU * (k + 1) as f32 / n as f32);
            let at = delay + sweep * k as f32 / n as f32;
            self.tris(parent, band(r - w * 0.5, r + w * 0.5, a0, a1), layer, at, gain, LILAC);
            if orbit {
                self.pieces.last_mut().unwrap().ang = 0.5 * (a0 + a1);
            }
        }
    }

    /// A character set upright at the origin, `size` tall; returns it with its advance.
    fn glyph(font: &[u8], ch: char, size: f32) -> (Option<Geometry>, f32) {
        match Geometry::text_glyphs(font, &ch.to_string(), 1.0, 0.01, 0.0).ok().and_then(|g| g.into_iter().next()) {
            Some(g) => (Some(g.geometry.scale(size, size, 1.0)), g.advance * size),
            None => (None, 0.3 * size),
        }
    }

    /// Characters spaced evenly round a ring, their feet toward the centre... of the page: the
    /// tops point outward. One piece per character, drawn in turn.
    #[allow(clippy::too_many_arguments)]
    fn text_on_ring(&mut self, parent: NodeId, chars: &[&str], rr: f32, size: f32, offset: f32, layer: f32, delay: f32, sweep: f32, gain: f32) {
        use std::f32::consts::TAU;
        let n = chars.len();
        for (i, ch) in chars.iter().enumerate() {
            let a = offset + TAU * i as f32 / n as f32;
            let Ok(geo) = Geometry::text(crate::world::lyric_font(), ch, 1.0, 0.01, 0.0) else { continue };
            let width = geo.bounds().size().x * size;
            let rot = Quat::from_rotation_z(a - deg(90.0));
            let p = polar(a, rr) + rot * Vec3::new(-width * 0.5, -size * 0.35, 0.0);
            let geo = geo.scale(size, size, 1.0).rotate(rot).translate(p.x, p.y, p.z);
            self.geo(parent, geo, layer, delay + sweep * i as f32 / n as f32, gain, LILAC);
        }
    }
}

impl MagicCircle {
    /// `credits`: write the vocal and music credits along the lower arc of the quiet band,
    /// starting this many seconds after the circle starts drawing.
    fn build(scene: &mut Scene, centre: Vec3, r: f32, credits: Option<f32>) -> Self {
        use std::f32::consts::TAU;
        let root = scene.add(Node::group().with_position(centre));
        let mut d = Draft { scene, pieces: Vec::new() };
        let mut spin = Vec::new();
        let hair = r * 0.0022;
        let up = deg(90.0);

        // 0: the sun. A bright heart, two rings and sixteen rays, long and short in turn.
        let sun = d.scene.add_to(root, Node::group().at(0.0, 0.0, 0.6));
        d.tris(sun, dot(Vec3::ZERO, r * 0.022), 0.0, 0.0, 1.6, WARM);
        d.ring(sun, r * 0.055, hair * 1.2, 0.0, 0.0, 0.3, up, 1.2, false);
        d.ring(sun, r * 0.092, hair * 1.6, 0.0, 0.05, 0.4, up, 1.2, false);
        for i in 0..16 {
            let a = TAU * i as f32 / 16.0;
            let tip = r * if i % 2 == 0 { 0.2 } else { 0.15 };
            let half = deg(if i % 2 == 0 { 7.0 } else { 5.0 });
            let mut t = stroke(polar(a - half, r * 0.105), polar(a, tip), hair);
            t.extend(stroke(polar(a + half, r * 0.105), polar(a, tip), hair));
            d.tris(sun, t, 0.3, 0.25 + 0.02 * i as f32, if i % 2 == 0 { 1.0 } else { 0.7 }, WARM);
        }
        spin.push((sun, 0.1));

        // 1: {12/5} star lattice in its ring, a node at every point.
        let lattice = d.scene.add_to(root, Node::group().at(0.0, 0.0, 0.36));
        d.ring(lattice, r * 0.26, hair, 1.0, 0.25, 0.7, up, 0.9, false);
        let lp: Vec<Vec3> = (0..12).map(|i| polar(TAU * i as f32 / 12.0, r * 0.26)).collect();
        for i in 0..12 {
            d.tris(lattice, stroke(lp[i], lp[(i + 5) % 12], hair * 0.8), 1.0, 0.4 + 0.05 * i as f32, 0.55, LILAC);
            let mut node = band(r * 0.013, r * 0.013 + hair, 0.0, TAU);
            node.extend(dot(Vec3::ZERO, r * 0.005));
            d.tris(lattice, node.into_iter().map(|p| p + lp[i]).collect(), 1.0, 0.4 + 0.05 * i as f32, 1.3, STAR);
        }
        spin.push((lattice, -0.07));

        // 2: the quiet band. It does not turn. The credits are written along its lower arc
        // and a row of dots fills the rest.
        let quiet = d.scene.add_to(root, Node::group().at(0.0, 0.0, 0.2));
        d.ring(quiet, r * 0.475, hair, 2.0, 0.45, 0.8, -up, 0.9, false);
        d.ring(quiet, r * 0.605, hair, 2.0, 0.5, 0.8, up, 0.9, false);
        let mut span = 0.0_f32;
        if let Some(at) = credits {
            // (text, size, tracking, gain); an empty text is the red diamond between the two.
            let runs: [(&str, f32, f32, f32); 5] = [
                ("VOCAL", 0.03, 0.42, 1.9),
                ("Nenne", 0.066, 0.14, 3.4),
                ("", 0.0, 0.0, 0.0),
                ("*Luna", 0.066, 0.14, 3.4),
                ("MUSIC", 0.03, 0.42, 1.9),
            ];
            let gap = |k: usize| r * if k == 1 || k == 2 { 0.075 } else { 0.045 };
            // Lay the runs out on a straight line first, then bend the line onto the arc.
            let mut placed: Vec<(Option<Geometry>, f32, f32, usize)> = Vec::new();
            let mut pen = 0.0;
            for (k, (text, size, track, gain)) in runs.iter().enumerate() {
                if text.is_empty() {
                    placed.push((None, pen, 0.0, k));
                } else {
                    for ch in text.chars() {
                        let (geo, adv) = Draft::glyph(crate::world::LABEL_FONT, ch, r * size);
                        placed.push((geo, pen, *gain, k));
                        pen += adv + r * size * track;
                    }
                    pen -= r * size * track;
                }
                if k + 1 < runs.len() {
                    pen += gap(k);
                }
            }
            let rb = r * 0.568;
            span = pen / rb;
            let a_start = -up - span * 0.5;
            let mut n = 0;
            for (geo, x, gain, k) in placed {
                let a = a_start + x / rb;
                let Some(geo) = geo else {
                    d.tris(quiet, diamond(polar(a, r * 0.54), a, r * 0.016, r * 0.007), 2.0, at + 0.3, 1.6, RED_HOT);
                    continue;
                };
                let p = polar(a, rb);
                let geo = geo.rotate_z(a + up).translate(p.x, p.y, 0.0);
                d.geo(quiet, geo, 2.0, at + 0.035 * n as f32 + if k > 2 { 0.25 } else { 0.0 }, gain, STAR);
                n += 1;
            }
            span += deg(9.0);
        }
        for sector in 0..24 {
            let mut t = Vec::new();
            for q in 0..5 {
                let i = sector * 5 + q;
                let a = up + TAU * i as f32 / 120.0;
                // Angle from straight down; the credits own the arc round it.
                let from_bottom = (a + up).rem_euclid(TAU);
                let from_bottom = from_bottom.min(TAU - from_bottom);
                if from_bottom < span * 0.5 {
                    continue;
                }
                if i % 5 == 0 {
                    t.extend(diamond(polar(a, r * 0.54), a, r * 0.011, r * 0.005));
                } else {
                    t.extend(dot(polar(a, r * 0.54), r * 0.0028));
                }
            }
            if !t.is_empty() {
                d.tris(quiet, t, 2.0, 0.6 + 0.035 * sector as f32, 0.9, LILAC);
            }
        }
        if credits.is_some() {
            for side in [-1.0_f32, 1.0] {
                let a = -up + side * span * 0.5;
                d.tris(quiet, diamond(polar(a, r * 0.54), a, r * 0.014, r * 0.006), 2.0, 1.4, 1.5, RED_HOT);
            }
        }

        // 3: five rune circles joined by arcs and arrows, inside a ring of twelve spikes.
        let pentad = d.scene.add_to(root, Node::group().at(0.0, 0.0, 0.0));
        let (pr, pc) = (r * 0.69, r * 0.048);
        for j in 0..5 {
            let a = up + TAU * j as f32 / 5.0;
            let c = polar(a, pr);
            let at = 0.7 + 0.08 * j as f32;
            d.tris(pentad, band(pc - hair * 0.6, pc + hair * 0.6, 0.0, TAU).into_iter().map(|p| p + c).collect(), 3.0, at, 1.1, LILAC);
            if let Ok(geo) = Geometry::text(crate::world::lyric_font(), ["時", "刻", "巡", "環", "夢"][j], 1.0, 0.01, 0.0) {
                let size = r * 0.048;
                let w = geo.bounds().size().x * size;
                d.geo(pentad, geo.scale(size, size, 1.0).translate(c.x - w * 0.5, c.y - size * 0.36, 0.0), 3.0, at + 0.15, 1.2, STAR);
            }
            // The arc on to the next circle, with an arrowhead halfway.
            let reach = (pc / pr).asin() * 1.25;
            let mut t = band(pr - hair * 0.4, pr + hair * 0.4, a + reach, a + TAU / 5.0 - reach);
            let b = a + TAU / 10.0;
            let tip = polar(b + 0.035, pr);
            t.extend(stroke(polar(b - 0.03, pr + r * 0.016), tip, hair));
            t.extend(stroke(polar(b - 0.03, pr - r * 0.016), tip, hair));
            d.tris(pentad, t, 3.0, at + 0.1, 0.7, LILAC);
        }
        d.ring(pentad, r * 0.75, hair, 3.0, 0.8, 0.8, up, 0.9, false);
        for i in 0..12 {
            let a = TAU * i as f32 / 12.0;
            let mut t = stroke(polar(a - 0.06, r * 0.75), polar(a, r * 0.795), hair);
            t.extend(stroke(polar(a + 0.06, r * 0.75), polar(a, r * 0.795), hair));
            d.tris(pentad, t, 3.0, 0.9 + 0.04 * i as f32, 0.9, LILAC);
        }
        spin.push((pentad, 0.12));

        // 4: clock numerals between two rings (they tick once a beat).
        let numerals = d.scene.add_to(root, Node::group().at(0.0, 0.0, -0.2));
        d.ring(numerals, r * 0.815, hair, 4.0, 0.9, 0.8, up, 0.9, false);
        d.ring(numerals, r * 0.887, hair, 4.0, 0.95, 0.8, -up, 0.9, false);
        d.text_on_ring(numerals, &["十二", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一"], r * 0.851, r * 0.042, up, 4.0, 1.1, -0.6, 1.2);

        // 5: a band of small time-runes.
        let runes = d.scene.add_to(root, Node::group().at(0.0, 0.0, -0.4));
        let rune_chars: Vec<String> = "時刻巡環永遠夢願星月鉄音".chars().cycle().take(48).map(|c| c.to_string()).collect();
        let rune_refs: Vec<&str> = rune_chars.iter().map(String::as_str).collect();
        d.text_on_ring(runes, &rune_refs, r * 0.915, r * 0.022, 0.0, 5.0, 1.2, 0.9, 0.8);
        spin.push((runes, 0.05));

        // 6: a graduated scale, like the rim of an astrolabe: a tick every two degrees, a
        // longer one every ten.
        let scale = d.scene.add_to(root, Node::group().at(0.0, 0.0, -0.6));
        d.ring(scale, r * 0.945, hair, 6.0, 1.1, 0.9, up, 0.9, false);
        for sector in 0..36 {
            let mut t = Vec::new();
            for q in 0..5 {
                let a = up + TAU * (sector * 5 + q) as f32 / 180.0;
                t.extend(stroke(polar(a, r * 0.95), polar(a, r * if q == 0 { 0.985 } else { 0.968 }), hair * 0.8));
            }
            d.tris(scale, t, 6.0, 1.2 + 0.025 * sector as f32, 0.8, LILAC);
        }
        spin.push((scale, -0.02));

        // 7: the outer ring (a light runs round it) and the eight phases of the moon riding
        // outside it, the lit part of each filled.
        let riders = d.scene.add_to(root, Node::group().at(0.0, 0.0, -0.8));
        d.ring(riders, r, hair * 2.2, 7.0, 1.2, 1.0, up, 1.2, true);
        for j in 0..8 {
            let a = up - TAU * j as f32 / 8.0;
            let mr = r * 0.04;
            let c = polar(a, r + mr + hair);
            let mut t = band(mr - hair * 0.5, mr + hair * 0.5, 0.0, TAU);
            // Phase j of 8: new, waxing crescent, first quarter ... The terminator is half an
            // ellipse; the lit part lies between it and the limb on the lit side.
            let phase = j as f32 / 8.0;
            let lit_right = phase < 0.5;
            let k = (phase * TAU).cos(); // 1 new, -1 full
            let side = if lit_right { 1.0 } else { -1.0 };
            for q in 0..16 {
                let y = |q: usize| -up + deg(180.0) * q as f32 / 16.0;
                let limb = |q: usize| Vec3::new(side * y(q).cos() * mr, y(q).sin() * mr, 0.0);
                let term = |q: usize| Vec3::new(side * y(q).cos() * mr * k, y(q).sin() * mr, 0.0);
                t.extend([limb(q), term(q), term(q + 1), limb(q), term(q + 1), limb(q + 1)]);
            }
            d.tris(riders, t.into_iter().map(|p| p + c).collect(), 7.0, 1.5 + 0.07 * j as f32, 0.42, STAR);
        }
        spin.push((riders, -0.03));

        // 8: a great eight-pointed star of two squares round everything, larger than the frame,
        // with a circle on each point and a faint ring through them.
        let star = d.scene.add_to(root, Node::group().at(0.0, 0.0, -1.4));
        let sr = r * 1.62;
        for sq in 0..2 {
            for i in 0..4 {
                let a = deg(45.0) * sq as f32 + TAU * i as f32 / 4.0;
                let (p, q) = (polar(a, sr), polar(a + TAU / 4.0, sr));
                // Stop short of the circles on the points.
                let (p, q) = (p.lerp(q, 0.03), q.lerp(p, 0.03));
                d.tris(star, stroke(p, q, hair * 1.2), 8.0, 1.5 + 0.1 * (sq * 4 + i) as f32, 0.5, LILAC);
                d.tris(star, band(r * 0.05 - hair * 0.5, r * 0.05 + hair * 0.5, 0.0, TAU).into_iter().map(|v| v + polar(a, sr)).collect(),
                    8.0, 1.6 + 0.1 * (sq * 4 + i) as f32, 1.0, LILAC);
                d.tris(star, dot(polar(a, sr), r * 0.012), 8.0, 1.6 + 0.1 * (sq * 4 + i) as f32, 1.4, STAR);
            }
        }
        d.ring(star, r * 1.146, hair, 8.0, 1.4, 1.0, 0.0, 0.5, false);
        d.ring(star, sr, hair, 8.0, 1.7, 1.2, 0.0, 0.5, true);
        spin.push((star, 0.015));

        let pieces = d.pieces;
        scene[root].visible = false;
        Self { root, pieces, spin, numerals }
    }

    /// `on` 0..1, `clock` the time it turns on (stops while time stops), `ticks` beats the
    /// numerals have advanced, `glow` base brightness, `pulse_since` seconds since a glow last
    /// set out from the centre, `appear_t` seconds since it started drawing.
    #[allow(clippy::too_many_arguments)]
    fn pose(&self, s: &mut Scene, on: f32, clock: f32, ticks: f32, glow: f32, pulse_since: f32, appear_t: f32) {
        use std::f32::consts::TAU;
        s[self.root].visible = on > 0.003;
        if !s[self.root].visible {
            return;
        }
        // The disc sways, so its layers (each at its own depth) slide against each other.
        s[self.root].rotation = Quat::from_rotation_x(0.04 * (clock * 0.37).sin()) * Quat::from_rotation_y(0.06 * (clock * 0.29 + 1.0).sin());
        let head = clock * 0.9;
        for p in &self.pieces {
            let since = appear_t - p.delay;
            let visible = since > 0.0;
            s[p.node].visible = visible;
            if !visible {
                continue;
            }
            // Drawn bright, then it settles.
            let drawn = ease_out(since / 0.25);
            let flash = (-since / 0.45).exp();
            // A glow that travels outward through the layers, then fades.
            let ps = pulse_since - p.layer * 0.09;
            let pulse = if ps >= 0.0 { (-ps / 0.45).exp() * ease_out(ps / 0.06) } else { 0.0 };
            let mut b = glow + 1.5 * pulse + 2.4 * flash;
            if p.ang.is_finite() {
                // The running light: bright at its head, a tail behind it.
                let behind = (head - p.ang).rem_euclid(TAU);
                b += 2.6 * (-behind / 0.7).exp();
            }
            if let Some(m) = s[p.node].material_mut() {
                m.color = Rgb::hex(p.color) * (on * drawn * p.gain * b);
            }
        }
        for (group, speed) in &self.spin {
            s[*group].rotation = Quat::from_rotation_z(clock * speed);
        }
        s[self.numerals].rotation = Quat::from_rotation_z(-deg(6.0) * ticks);
    }
}

/// What each family of elements should show at a time.
pub struct Mix {
    pub stars: f32,
    pub dial: f32,
    pub gem: f32,
    pub title_circle: f32,
    pub ruler: f32,
}

/// On between `a` and `b`, with fades of `fade` seconds.
pub fn window(t: f32, a: f32, b: f32, fade: f32) -> f32 {
    ease_between(t, a - fade, a) * (1.0 - ease_between(t, b, b + fade))
}

/// The final pull back shows everything faintly.
pub fn finale(_t: f32) -> f32 {
    0.0
}

pub fn mix(t: f32) -> Mix {
    let f = finale(t) * 0.6;
    // Constellations: drawn in the intro, then they stay as the sky of the whole song.
    let stars = 0.3 + 0.7 * window(t, 0.0, VERSE_1, 1.5) + 0.5 * window(t, VERSE_2 - 1.0, BAND_BACK, 1.0)
        + 0.6 * window(t, BREAKDOWN, FINAL, 1.0);
    let dial = (window(t, CHORUS_1, 88.6, 0.05) * (1.0 - window(t, KURUSHIKU - 0.1, TIME_STOP.1, 0.25))
        * (1.0 - window(t, 153.1, 159.4, 0.25))
        + window(t, CHORUS_2, BRIDGE - 1.0, 0.05)
        + window(t, FINAL, 249.0, 0.05) * 0.6)
        .max(f);
    let gem = window(t, BRIDGE - 0.8, BREAKDOWN + 0.5, 1.2).max(f);
    let title_circle = window(t, BAND_IN - 0.05, VERSE_1 - 0.5, 0.8);
    let ruler = window(t, PRE_1 + 0.2, BLUE_WORLD - 0.9, 0.4);
    // The circles are no longer the subject: the title keeps a faint one, the choruses none.
    let (dial, title_circle, ruler) = (dial * 0.0, title_circle * 0.5, ruler * 0.0);
    Mix { stars, dial, gem, title_circle, ruler }
}

impl Graphics {
    pub fn build(scene: &mut Scene) -> Self {
        let link_geo = std::sync::Arc::new(Geometry::cuboid(0.016, 1.0, 0.016).translate(0.0, 0.5, 0.0));
        let star_geo = std::sync::Arc::new(Geometry::circle(1.0, 24));

        // Constellations: real figures. Each edge draws on its note; a star lights the first
        // time an edge reaches it.
        let mut stars = Vec::new();
        let mut links = Vec::new();
        for (c, centre, scale, times) in constellation_plan() {
            let (_, catalogue, edges) = SKY[c];
            let ra0 = catalogue.iter().map(|s| s.0).sum::<f32>() / catalogue.len() as f32;
            let dec0 = catalogue.iter().map(|s| s.1).sum::<f32>() / catalogue.len() as f32;
            let pos: Vec<Vec3> = catalogue
                .iter()
                .enumerate()
                .map(|(k, (ra, dec, _))| {
                    // Seen from inside the sphere: east (larger RA) is to the left.
                    let x = -(ra - ra0) * 15.0 * deg(dec0).cos();
                    let y = dec - dec0;
                    centre + Vec3::new(x * scale, y * scale, (hash(k as u32, c as u32) - 0.5) * 2.0)
                })
                .collect();
            let mut first = vec![f32::MAX; catalogue.len()];
            for (k, (a, b)) in edges.iter().enumerate() {
                let at = times[k.min(times.len() - 1)];
                first[*a] = first[*a].min(at);
                first[*b] = first[*b].min(at + 0.25);
                let d = pos[*b] - pos[*a];
                let id = scene.add(
                    Node::mesh(link_geo.clone(), line_mat(LILAC))
                        .with_position(pos[*a])
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()))
                        .hidden(),
                );
                links.push(Link { node: id, len: d.length(), at, thick: if c >= 6 { 1.9 } else { 1.0 } });
            }
            for (k, (_, _, mag)) in catalogue.iter().enumerate() {
                // Brighter stars are bigger (magnitude is backwards: smaller = brighter).
                let size = (0.03 + 0.05 * clamp01((4.5 - mag) / 4.5)) * if c >= 6 { 1.6 } else { 1.0 };
                let node = scene.add(
                    Node::mesh(star_geo.clone(), Material::glow(Rgb::hex(STAR), 0.0).double_sided())
                        .with_position(pos[k])
                        .scaled(size)
                        .hidden(),
                );
                stars.push(Star { node, at: first[k], size, red: c == 7 && k == 0 });
            }
        }

        // 高く高く: altitude ticks on both sides of the climb.
        let mut ruler = Vec::new();
        for k in 0..70 {
            let y = -4.0 + k as f32 * 0.6;
            for side in [-1.0_f32, 1.0] {
                let long = k % 5 == 0;
                let x = side * 5.2;
                let len = if long { 0.9 } else { 0.4 };
                let id = scene.add(line(
                    Vec3::new(x, y, 8.0),
                    Vec3::new(x - side * len, y, 8.0),
                    if long { 0.03 } else { 0.016 },
                    line_mat(LILAC),
                ));
                ruler.push((id, y));
            }
        }

        // Bridge: a gem-cut diagram drawn in hairlines.
        let gem = scene.add(Node::group().with_position(crate::world::GEM));
        let ring_pts = |r: f32, y: f32, n: usize| -> Vec<Vec3> {
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    Vec3::new(a.cos() * r, y, a.sin() * r)
                })
                .collect()
        };
        let table = ring_pts(2.2, 2.0, 8);
        let girdle = ring_pts(4.6, 0.6, 16);
        let culet = Vec3::new(0.0, -4.6, 0.0);
        for k in 0..8 {
            scene.add_to(gem, line(table[k], table[(k + 1) % 8], 0.035, line_mat(LILAC)));
            scene.add_to(gem, line(table[k], girdle[2 * k], 0.03, line_mat(LILAC)));
            scene.add_to(gem, line(table[k], girdle[(2 * k + 1) % 16], 0.03, line_mat(LILAC)));
            scene.add_to(gem, line(girdle[2 * k], culet, 0.025, line_mat(LILAC)));
        }
        for k in 0..16 {
            scene.add_to(gem, line(girdle[k], girdle[(k + 1) % 16], 0.035, line_mat(LILAC)));
        }

        // Magic circles.
        let title_circle = MagicCircle::build(scene, Vec3::new(0.0, 1.0, 6.5), 7.6, Some(CREDITS_AT - BAND_IN));
        let chorus_circle = MagicCircle::build(scene, Vec3::new(0.0, 0.0, 1.4), DIAL_R * 1.25, None);

        // Sound rings.
        let rings = (0..6)
            .map(|_| {
                scene.add(
                    Node::mesh(Geometry::ring(0.995, 1.0, 256), Material::glow(Rgb::hex(LILAC), 0.0).double_sided())
                        .at(0.0, 0.0, 0.5)
                        .hidden(),
                )
            })
            .collect();
        // Sound rings only for the iron (鉄) and the last chorus's hit.
        let ring_events: Vec<(f32, f32)> = vec![];

        Self { stars, links, gem, title_circle, title_circle_gain: 1.0, chorus_circle, ruler, rings, ring_events }
    }

    pub fn pose(&self, s: &mut Scene, t: f32, clock: f32, spell_since: f32, audio: &Audio) {
        let mut m = mix(t);
        m.title_circle *= self.title_circle_gain;
        let set = |s: &mut Scene, id: NodeId, op: f32| {
            s[id].visible = op > 0.003;
            for d in std::iter::once(id).chain(s.descendants(id)) {
                if let Some(mat) = s[d].material_mut() {
                    mat.opacity = op;
                }
            }
        };

        // Constellations: a soft glow on the note (not a flare), then a slow twinkle.
        for (k, st) in self.stars.iter().enumerate() {
            let since = t - st.at;
            s[st.node].visible = since >= 0.0;
            if since < 0.0 {
                continue;
            }
            let blip = (-since / 0.5).exp() * ease_out(since / 0.08);
            let twinkle = 0.85 + 0.15 * (clock * (0.9 + hash(k as u32, 5) * 1.2) + k as f32).sin();
            let glow = m.stars * (0.9 * twinkle * (0.9 + 0.2 * audio.high) + 1.8 * blip);
            s[st.node].scale = Vec3::splat(st.size * (1.0 + 0.5 * blip));
            if let Some(mat) = s[st.node].material_mut() {
                mat.color = if st.red { Rgb::hex(0xff6a6a) * (glow * 1.6) } else { Rgb::hex(STAR) * glow };
            }
        }
        for l in &self.links {
            let since = t - l.at;
            s[l.node].visible = since >= 0.0;
            if since < 0.0 {
                continue;
            }
            let blip = (-since / 0.5).exp();
            s[l.node].scale = Vec3::new(l.thick, l.len * expo_out(since / 0.9), l.thick);
            if let Some(mat) = s[l.node].material_mut() {
                mat.opacity = m.stars * (0.22 + 0.2 * blip);
            }
        }

        for (id, y) in &self.ruler {
            let on = m.ruler * ease_out((t - PRE_1 + 0.4 - (y + 4.0) * 0.01) / 0.5);
            set(s, *id, on * if (y + 4.0).rem_euclid(3.0) < 0.1 { 0.6 } else { 0.32 });
        }
        set(s, self.gem, 0.0);
        s[self.gem].rotation = Quat::from_rotation_y(t * 0.12) * Quat::from_rotation_x(deg(12.0));
        let tint = Rgb::hex(0x3fe0a6).lerp(Rgb::hex(0xc39bff), ease_between(t, BRIDGE_MOON - 1.5, BRIDGE_MOON));
        let tint = Rgb::hex(LILAC).lerp(tint, window(t, BRIDGE, BREAKDOWN, 1.5));
        for d in s.descendants(self.gem) {
            if let Some(mat) = s[d].material_mut() {
                mat.color = tint;
            }
        }

        // Magic circles. The numeral ring ticks once per beat (snapping, like a clock); the
        // inner layers turn on `clock`, which stops while time stops.
        let beats = if clock < DOWNBEAT_0 { 0.0 } else {
            let n = (clock - DOWNBEAT_0) / BEAT;
            n.floor() + expo_out(n.fract() / 0.35)
        };
        // Dim by default; a glow travels through on every other downbeat (and when a spell
        // word is inscribed in the chorus).
        let bar_since = if clock < DOWNBEAT_0 { 99.0 } else { ((clock - DOWNBEAT_0) / (2.0 * BAR)).fract() * 2.0 * BAR };
        let pulse_since = bar_since.min(spell_since);
        self.title_circle.pose(s, m.title_circle, clock, beats, 0.45 + 0.25 * audio.low, pulse_since, t - BAND_IN);
        let chorus_start = [CHORUS_1, CHORUS_2, FINAL].iter().copied().filter(|c| *c <= t + 0.01).last().unwrap_or(CHORUS_1);
        self.chorus_circle.pose(s, m.dial, clock, beats, 0.24 + 0.16 * audio.low, pulse_since, t - chorus_start);

        const RING_LIFE: f32 = 2.8;
        let active: Vec<&(f32, f32)> = self.ring_events.iter().filter(|(st, _)| t >= *st && t < st + RING_LIFE).collect();
        for (slot, id) in self.rings.iter().enumerate() {
            let Some((start, strength)) = active.get(slot) else {
                s[*id].visible = false;
                continue;
            };
            let p = (t - start) / RING_LIFE;
            s[*id].visible = true;
            s[*id].scale = Vec3::splat(1.5 + 24.0 * ease_out(p));
            if let Some(mat) = s[*id].material_mut() {
                mat.color = Rgb::hex(LILAC) * (strength * 1.5 * (1.0 - p).powi(2));
            }
        }
    }
}
