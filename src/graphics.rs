//! Flat graphic elements laid out in space: constellations, magic circles, hairline grids,
//! rings and altitude ticks. No solid objects; the depth comes from the camera.
use crate::timing::*;
use fframes::three_d::*;

pub const LILAC: u32 = 0x9d93d6;
pub const RED_HOT: u32 = 0xff5a6e;
const STAR: u32 = 0xf2ecff;

/// The chorus dial (the lyric stations sit on it).
pub const DIAL_R: f32 = 9.6;
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
    chorus_circle: MagicCircle,
    /// 高く高く: altitude ticks streaming past as the camera climbs.
    ruler: Vec<(NodeId, f32)>,
    rings: Vec<NodeId>,
    ring_events: Vec<(f32, f32)>,
}

/// A magic circle that "controls time", after the references: an outer ring with eight
/// small circles riding it, a hatched band, a band of small time-runes, the clock numerals
/// (they tick once a beat), a ring of outward spikes with five small circles and arrows, a
/// {12/5} star lattice and a double-ringed core with crescents. Layers turn at different
/// speeds and directions. It stays dim; a glow travels outward through it now and then.
struct MagicCircle {
    root: NodeId,
    /// (layer group, order from the centre).
    layers: Vec<(NodeId, f32)>,
    numerals: NodeId,
    runes: NodeId,
    lattice: NodeId,
    pentad: NodeId,
    riders: NodeId,
}

/// Arc of hairline segments from angle `a0` to `a1` (radians).
fn arc(scene: &mut Scene, parent: NodeId, r: f32, a0: f32, a1: f32, w: f32, mat: &dyn Fn() -> Material) {
    let n = ((a1 - a0).abs() * r * 3.0).ceil().max(4.0) as usize;
    let p = |k: usize| {
        let a = a0 + (a1 - a0) * k as f32 / n as f32;
        Vec3::new(a.cos() * r, a.sin() * r, 0.0)
    };
    for k in 0..n {
        scene.add_to(parent, line(p(k), p(k + 1), w, mat()));
    }
}

impl MagicCircle {
    fn build(scene: &mut Scene, centre: Vec3, r: f32) -> Self {
        use std::f32::consts::{FRAC_PI_2 as PI_2, PI as PI_1, TAU};
        let root = scene.add(Node::group().with_position(centre));
        let glow = || Material::glow(Rgb::hex(LILAC), 0.0).double_sided();
        let ring = |scene: &mut Scene, parent: NodeId, rr: f32, w: f32| {
            scene.add_to(parent, Node::mesh(Geometry::ring(rr - w, rr + w, 200), glow()));
        };
        let text_on_ring = |scene: &mut Scene, parent: NodeId, chars: &[&str], rr: f32, size: f32, offset: f32| {
            let n = chars.len();
            for (i, ch) in chars.iter().enumerate() {
                let a = offset + TAU * i as f32 / n as f32;
                let Ok(geo) = Geometry::text(crate::world::FONT_BYTES, ch, 1.0, 0.01, 0.0) else { continue };
                let width = geo.bounds().size().x * size;
                let rot = Quat::from_rotation_z(a - deg(90.0));
                let p = Vec3::new(a.cos() * rr, a.sin() * rr, 0.0);
                scene.add_to(
                    parent,
                    Node::mesh(geo, glow()).with_position(p + rot * Vec3::new(-width * 0.5, -size * 0.35, 0.0)).with_rotation(rot).scaled(size),
                );
            }
        };
        let mut layers = Vec::new();

        // 0: core — double ring and two crescents.
        let core = scene.add_to(root, Node::group().at(0.0, 0.0, 0.3));
        ring(scene, core, r * 0.1, 0.012);
        ring(scene, core, r * 0.075, 0.008);
        arc(scene, core, r * 0.16, deg(200.0), deg(340.0), 0.014, &glow);
        arc(scene, core, r * 0.16, deg(20.0), deg(160.0), 0.014, &glow);
        layers.push((core, 0.0));

        // 1: {12/5} star lattice in its ring.
        let lattice = scene.add_to(root, Node::group().at(0.0, 0.0, 0.18));
        ring(scene, lattice, r * 0.42, 0.01);
        let lp: Vec<Vec3> = (0..12).map(|i| { let a = TAU * i as f32 / 12.0; Vec3::new(a.cos(), a.sin(), 0.0) * r * 0.42 }).collect();
        for i in 0..12 {
            scene.add_to(lattice, line(lp[i], lp[(i + 5) % 12], 0.009, glow()));
            // A node at every point of the star.
            scene.add_to(lattice, Node::mesh(Geometry::ring(r * 0.012, r * 0.02, 32), glow()).with_position(lp[i]));
        }
        layers.push((lattice, 1.0));

        // 2: five small circles with arrows between, inside a ring of twelve outward spikes.
        let pentad = scene.add_to(root, Node::group().at(0.0, 0.0, 0.06));
        for j in 0..5 {
            let a = deg(90.0) + TAU * j as f32 / 5.0;
            let c = Vec2::from_angle(a) * r * 0.54;
            scene.add_to(pentad, Node::mesh(Geometry::ring(r * 0.06 - 0.01, r * 0.06 + 0.01, 72), glow()).at(c.x, c.y, 0.0));
            // A rune in each circle (one per corner of the pentad).
            if let Ok(geo) = Geometry::text(crate::world::FONT_BYTES, ["時", "刻", "巡", "環", "夢"][j], 1.0, 0.01, 0.0) {
                let size = r * 0.06;
                let w = geo.bounds().size().x * size;
                scene.add_to(pentad, Node::mesh(geo, glow()).at(c.x - w * 0.5, c.y - size * 0.36, 0.0).scaled(size));
            }
            let b = a + TAU / 10.0;
            let tip = Vec3::new(b.cos(), b.sin(), 0.0) * r * 0.62;
            let l = Vec3::new((b - 0.09).cos(), (b - 0.09).sin(), 0.0) * r * 0.5;
            let rr = Vec3::new((b + 0.09).cos(), (b + 0.09).sin(), 0.0) * r * 0.5;
            scene.add_to(pentad, line(l, tip, 0.01, glow()));
            scene.add_to(pentad, line(rr, tip, 0.01, glow()));
        }
        ring(scene, pentad, r * 0.66, 0.01);
        for i in 0..12 {
            let a = TAU * i as f32 / 12.0;
            let d = |o: f32, rr: f32| Vec3::new((a + o).cos(), (a + o).sin(), 0.0) * rr;
            scene.add_to(pentad, line(d(-0.07, r * 0.66), d(0.0, r * 0.72), 0.01, glow()));
            scene.add_to(pentad, line(d(0.07, r * 0.66), d(0.0, r * 0.72), 0.01, glow()));
        }
        layers.push((pentad, 2.0));

        // 3: clock numerals between two rings (they tick once a beat).
        let numerals = scene.add_to(root, Node::group().at(0.0, 0.0, 0.0));
        ring(scene, numerals, r * 0.74, 0.01);
        ring(scene, numerals, r * 0.82, 0.01);
        text_on_ring(scene, numerals, &["十二", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一"], r * 0.78, r * 0.045, deg(90.0));
        layers.push((numerals, 3.0));

        // 4: a band of small time-runes.
        let runes = scene.add_to(root, Node::group().at(0.0, 0.0, -0.12));
        let rune_chars: Vec<&str> = "時刻巡環永遠夢願星月鉄音".chars().cycle().take(48).map(|c| {
            // leak-free: index into a static string
            let s: &'static str = Box::leak(c.to_string().into_boxed_str());
            s
        }).collect();
        text_on_ring(scene, runes, &rune_chars, r * 0.86, r * 0.026, 0.0);
        ring(scene, runes, r * 0.895, 0.008);
        layers.push((runes, 4.0));

        // 5: hatched band.
        let hatch = scene.add_to(root, Node::group().at(0.0, 0.0, -0.22));
        ring(scene, hatch, r * 0.91, 0.01);
        ring(scene, hatch, r * 0.95, 0.01);
        for i in 0..144 {
            let a = TAU * i as f32 / 144.0;
            let d = Vec3::new(a.cos(), a.sin(), 0.0);
            scene.add_to(hatch, line(d * r * 0.915, d * r * 0.945, 0.008, glow()));
        }
        layers.push((hatch, 5.0));

        // 6: outer ring with eight small circles riding on it.
        let riders = scene.add_to(root, Node::group().at(0.0, 0.0, -0.34));
        ring(scene, riders, r, 0.014);
        for j in 0..8 {
            let a = TAU * j as f32 / 8.0;
            let c = Vec2::from_angle(a) * r * 0.975;
            // Moon ring: the eight phases, each a circle with its terminator arc.
            let moon = scene.add_to(riders, Node::group().at(c.x, c.y, 0.0));
            let mr = r * 0.045;
            scene.add_to(moon, Node::mesh(Geometry::ring(mr - 0.01, mr + 0.01, 72), glow()));
            let phase = j as f32 / 8.0;
            let bulge = (phase * TAU).cos();
            let pts: Vec<Vec3> = (0..=16)
                .map(|q| {
                    let a = -PI_2 + PI_1 * q as f32 / 16.0;
                    Vec3::new(a.cos() * mr * bulge, a.sin() * mr, 0.0)
                })
                .collect();
            for q in 0..16 {
                scene.add_to(moon, line(pts[q], pts[q + 1], 0.012, glow()));
            }
        }
        layers.push((riders, 6.0));

        scene[root].visible = false;
        Self { root, layers, numerals, runes, lattice, pentad, riders }
    }

    /// `on` 0..1, `clock` the time it turns on (stops while time stops), `ticks` beats the
    /// numerals have advanced, `glow` base brightness (dim), `pulse` the travelling glow
    /// time since the last pulse began (seconds), `appear_t` time since it started drawing.
    fn pose(&self, s: &mut Scene, on: f32, clock: f32, ticks: f32, glow: f32, pulse_since: f32, appear_t: f32) {
        s[self.root].visible = on > 0.003;
        if !s[self.root].visible {
            return;
        }
        for (layer, k) in &self.layers {
            let d = expo_out(clamp01((appear_t - k * 0.1) / 1.0));
            // A glow that travels outward through the layers, then fades.
            let since = pulse_since - k * 0.09;
            let pulse = if since >= 0.0 { (-since / 0.45).exp() * ease_out(since / 0.06) } else { 0.0 };
            s[*layer].scale = Vec3::splat(0.8 + 0.2 * d);
            let c = Rgb::hex(LILAC) * (on * d * (glow + 1.6 * pulse));
            for id in std::iter::once(*layer).chain(s.descendants(*layer)) {
                if let Some(m) = s[id].material_mut() {
                    if m.blending == Blending::Additive {
                        m.color = c;
                    } else {
                        m.opacity = on * d;
                    }
                }
            }
        }
        s[self.numerals].rotation = Quat::from_rotation_z(-deg(6.0) * ticks);
        s[self.runes].rotation = Quat::from_rotation_z(clock * 0.05);
        s[self.lattice].rotation = Quat::from_rotation_z(-clock * 0.07);
        s[self.pentad].rotation = Quat::from_rotation_z(clock * 0.12);
        s[self.riders].rotation = Quat::from_rotation_z(-clock * 0.03);
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
    let (dial, title_circle, ruler) = (dial * 0.0, title_circle * 0.3, ruler * 0.0);
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
        let title_circle = MagicCircle::build(scene, Vec3::new(0.0, 1.0, 9.0), 6.4);
        let chorus_circle = MagicCircle::build(scene, Vec3::new(0.0, 0.0, 1.4), DIAL_R * 1.25);

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

        Self { stars, links, gem, title_circle, chorus_circle, ruler, rings, ring_events }
    }

    pub fn pose(&self, s: &mut Scene, t: f32, clock: f32, spell_since: f32, audio: &Audio) {
        let m = mix(t);
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
