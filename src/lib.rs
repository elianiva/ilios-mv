//! イリオス (ねんね / *Luna): lyric video prototype, 0:00–1:40.
//!
//! The whole video is one continuous 3D world (`world.rs`) rendered under an SVG frame that
//! follows the album cover: hairline grid, small serif labels, vertical credits.
//! Scenes only name the song sections for the CLI (`strip Chorus`, `preview Verse`).
pub mod graphics;
pub mod lyrics;
pub mod timing;
pub mod world;

use fframes::{
    AudioMap, AudioTimestamp::Second, AudioTrack, Color, Duration, FFramesContext, Frame, SampleSize, Scene,
    Scenes, Shader, ShaderUniforms, Svgr, Video, VisualizeFrameInput, WindowFunction, include_media_dir,
};
use timing::*;

include_media_dir!(pub struct IliosMedia, "media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;

const SERIF: &str = "Shippori Mincho";
const TEXT: &str = "#e4d2d7";
const RED: &str = "#c53f3b";

/// A named stretch of the song. Draws nothing itself; the video draws the world.
#[derive(Debug)]
pub struct Section {
    name: &'static str,
    seconds: f32,
}

impl Scene for Section {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(self.seconds)
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}

pub struct IliosVideo<'a> {
    pub media: &'a IliosMedia,
    world: world::World,
    post: Shader,
    sections: [Section; 13],
}

impl<'a> IliosVideo<'a> {
    pub fn new(media: &'a IliosMedia) -> Self {
        let lines = lyrics::parse(include_str!("../lyrics/ilios.lrc"));
        let bounds = [
            0.0, VERSE_1, PRE_1, CHORUS_1, INTERLUDE, VERSE_2, BAND_BACK, CHORUS_2, BRIDGE, BREAKDOWN, FINAL, 248.6, OUTRO, END,
        ];
        let names = [
            "Intro", "Verse", "PreChorus", "Chorus", "Interlude", "VerseTwo", "PreChorusTwo", "ChorusTwo", "Bridge",
            "Breakdown", "FinalChorus", "Afterglow", "Outro",
        ];
        let sections = std::array::from_fn(|i| Section { name: names[i], seconds: bounds[i + 1] - bounds[i] });
        Self {
            media,
            world: world::World::new(&lines, WIDTH as u32, HEIGHT as u32),
            post: Shader::sksl(include_str!("shaders/post.sksl")),
            sections,
        }
    }
}

impl std::fmt::Debug for IliosVideo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IliosVideo")
    }
}

/// Bands of the song at this frame from fframes' FFT (1024 samples, Hann, a little smoothing),
/// compressed to roughly 0..1. Calibrated on this mix: the sustained wall from 0:15.8
/// sits around 0.6–0.8 in the low band.
fn audio_features(frame: &Frame, ctx: &FFramesContext<'_, '_>) -> graphics::Audio {
    let Some(data) = ctx.get_audio("audio.flac") else { return graphics::Audio::default() };
    let spectrum = frame.visualize_audio_frame(VisualizeFrameInput {
        audio: data,
        sample_size: SampleSize::S1024,
        smooth_level: 2,
        window: Some(WindowFunction::Hann),
    });
    if spectrum.len() < 300 {
        return graphics::Audio::default();
    }
    let mean = |a: usize, b: usize| spectrum[a..b.min(spectrum.len())].iter().sum::<f32>() / (b.min(spectrum.len()) - a).max(1) as f32;
    // Each band against its own loud reference (measured on the chorus wall), so quiet
    // passages read near 0, the wall near 1, and swells move in between.
    let level = |a: usize, b: usize, reference: f32| -> f32 {
        ((1.0 + mean(a, b)).ln() / (1.0 + reference).ln()).clamp(0.0, 1.3).powf(1.6)
    };
    // 32 log-spaced bands from ~43 Hz to ~11 kHz; the reference falls with frequency.
    let bins = (0..32)
        .map(|k| {
            let a = 256.0_f32.powf(k as f32 / 32.0) as usize;
            let b = (256.0_f32.powf((k + 1) as f32 / 32.0) as usize).max(a + 1);
            level(a, b, 40.0 * 0.88_f32.powi(k))
        })
        .collect();
    let out = graphics::Audio { low: level(1, 6, 38.0), mid: level(6, 60, 7.5), high: level(60, 250, 2.2), bins };
    if std::env::var_os("ILIOS_AUDIO_DEBUG").is_some() {
        eprintln!("t={:.2} raw low={:.2} mid={:.2} high={:.2} -> {:.2} {:.2} {:.2}", frame.seconds(), mean(1, 6), mean(6, 60), mean(60, 250), out.low, out.mid, out.high);
    }
    out
}


/// Section names for the frame's top-left label.
const SECTION_LABELS: &[(f32, &str)] = &[
    (0.0, "INTRO"), (VERSE_1, "VERSE I"), (PRE_1, "PRE-CHORUS"), (CHORUS_1, "CHORUS"), (INTERLUDE, "INTERLUDE"),
    (VERSE_2, "VERSE II"), (BAND_BACK, "PRE-CHORUS II"), (CHORUS_2, "CHORUS II"), (BRIDGE, "BRIDGE"),
    (BREAKDOWN, "BREAKDOWN"), (FINAL, "LAST CHORUS"), (248.6, "AFTERGLOW"), (OUTRO, "OUTRO"),
];

/// The quiet frame round the picture: corner marks, edge rulers with the song's progress,
/// the section, a bar/beat counter in 3/4, and the constellation being drawn.
fn hud<'a>(t: f32, op: f32) -> Svgr<'a> {
    if op <= 0.003 {
        return Svgr::empty();
    }
    let (idx, section) = SECTION_LABELS.iter().enumerate().filter(|(_, (a, _))| *a <= t).map(|(i, (_, n))| (i, *n)).last().unwrap_or((0, "INTRO"));
    let beat_n = ((t - DOWNBEAT_0) / BEAT).floor();
    let (bar, beat) = if t < DOWNBEAT_0 { (0, 0) } else { ((beat_n / 3.0) as i32 + 1, (beat_n as i32).rem_euclid(3)) };
    let beat_flash = if t < DOWNBEAT_0 { 0.0 } else { (-(((t - DOWNBEAT_0) / BEAT).fract() * BEAT) / 0.25).exp() };
    let mins = (t / 60.0) as i32;
    let secs = t - mins as f32 * 60.0;
    let timecode = format!("{:02}:{:05.2}", mins, secs);
    let counter = format!("BAR {:03}", bar);
    let label = format!("{:02}  —  {}", idx + 1, section);
    let progress = 180.0 + 720.0 * clamp01(t / END);

    // The constellation being drawn, with its catalogue position.
    let mut sky_label = String::new();
    let mut sky_coord = String::new();
    let mut sky_op = 0.0_f32;
    for (c, _, _, times) in graphics::constellation_plan() {
        let w = graphics::window(t, times[0] - 0.3, times[times.len() - 1] + 2.0, 0.4);
        if w > sky_op {
            sky_op = w;
            let (name, stars, _) = graphics::SKY[c];
            let ra = stars.iter().map(|s| s.0).sum::<f32>() / stars.len() as f32;
            let dec = stars.iter().map(|s| s.1).sum::<f32>() / stars.len() as f32;
            sky_label = name.to_string();
            sky_coord = format!(
                "RA {:02}h{:02}m   DEC {}{:02}°{:02}′",
                ra as i32,
                ((ra.fract()) * 60.0) as i32,
                if dec < 0.0 { "−" } else { "+" },
                dec.abs() as i32,
                (dec.abs().fract() * 60.0) as i32
            );
        }
    }
    let credits = 1.0 - sky_op;

    let corner = |x: f32, y: f32, sx: f32, sy: f32| {
        fframes::svgr!(<path d={format!("M {} {} L {} {} L {} {}", x + sx * 34.0, y, x, y, x, y + sy * 34.0)}
            fill="none" stroke={TEXT} stroke-width="1.4" stroke-opacity="0.4" />)
    };
    let ticks: Vec<Svgr> = (0..=24)
        .flat_map(|k| {
            let y = 180.0 + k as f32 * 30.0;
            let len = if k % 5 == 0 { 14.0 } else { 7.0 };
            [
                fframes::svgr!(<line x1="64" y1={y} x2={64.0 + len} y2={y} stroke={TEXT} stroke-width="1" stroke-opacity="0.22" />),
                fframes::svgr!(<line x1="1856" y1={y} x2={1856.0 - len} y2={y} stroke={TEXT} stroke-width="1" stroke-opacity="0.22" />),
            ]
        })
        .collect();
    let dots: Vec<Svgr> = (0..3)
        .map(|k| {
            let on = k == beat;
            let r = if on { 4.0 + 2.0 * beat_flash } else { 3.0 };
            fframes::svgr!(<circle cx={1748.0 + k as f32 * 18.0} cy="112" r={r} fill={if on { TEXT } else { "none" }}
                stroke={TEXT} stroke-width="1" fill-opacity={0.5 + 0.4 * beat_flash} stroke-opacity="0.5" />)
        })
        .collect();

    fframes::svgr!(
        <g opacity={op}>
            {corner(56.0, 56.0, 1.0, 1.0)}
            {corner(1864.0, 56.0, -1.0, 1.0)}
            {corner(56.0, 1024.0, 1.0, -1.0)}
            {corner(1864.0, 1024.0, -1.0, -1.0)}
            <line x1="64" y1="180" x2="64" y2="900" stroke={TEXT} stroke-width="1" stroke-opacity="0.18" />
            <line x1="1856" y1="180" x2="1856" y2="900" stroke={TEXT} stroke-width="1" stroke-opacity="0.18" />
            {ticks}
            <path d={format!("M 1846 {} L 1834 {} L 1834 {} Z", progress, progress - 6.0, progress + 6.0)} fill={RED} fill-opacity="0.8" />
            <text x="104" y="118" font-family={SERIF} font-weight="500" font-size="24" letter-spacing="6" fill={TEXT} fill-opacity="0.62">{label}</text>
            <text x="1724" y="120" text-anchor="end" font-family={SERIF} font-weight="500" font-size="24" letter-spacing="4" fill={TEXT} fill-opacity="0.55">{counter}</text>
            {dots}
            <text x="1816" y="160" text-anchor="end" font-family={SERIF} font-weight="500" font-size="24" letter-spacing="3" fill={TEXT} fill-opacity="0.4">{timecode}</text>
            <g opacity={sky_op}>
                <text x="104" y="972" font-family={SERIF} font-weight="500" font-size="26" letter-spacing="8" fill={TEXT} fill-opacity="0.7">{sky_label}</text>
                <text x="104" y="1008" font-family={SERIF} font-weight="500" font-size="24" letter-spacing="3" fill={TEXT} fill-opacity="0.45">{sky_coord}</text>
            </g>
            <g opacity={credits}>
                <text x="104" y="1004" font-family={SERIF} font-weight="500" font-size="24" letter-spacing="6" fill={TEXT} fill-opacity="0.5">"NENNE  /  *LUNA"</text>
            </g>
        </g>
    )
}

/// The album cover's framing: four hairlines that cross near two corners, the title small at
/// the bottom right with the cover's red rule, and the credits set vertically on the sides.
/// `bar` is the letterbox height in pixels. The frame is laid out on the picture between the
/// bars, so it grows and shrinks with them and never draws on the black.
fn cover_frame<'a>(t: f32, op: f32, bar: f32) -> Svgr<'a> {
    let draw = |from: f32| ease_between(t, BAND_IN + from, BAND_IN + from + 2.4);
    if op <= 0.003 || t < BAND_IN {
        return Svgr::empty();
    }
    let labels = ease_out((t - VERSE_1) / 1.2);
    // A height on the full 1080 frame, mapped onto the picture between the bars.
    let y = |v: f32| bar + v * (1080.0 - 2.0 * bar) / 1080.0;
    let (top, bottom) = (y(62.0), y(1018.0));
    let hair = |x1: f32, y1: f32, x2: f32, y2: f32, k: f32| {
        fframes::svgr!(<line x1={x1} y1={y1} x2={x1 + (x2 - x1) * k} y2={y1 + (y2 - y1) * k}
            stroke={TEXT} stroke-width="1" stroke-opacity="0.2" />)
    };
    fframes::svgr!(
        <g opacity={op}>
            {hair(0.0, top, 820.0, top, draw(0.0))}
            {hair(150.0, y(0.0), 150.0, y(700.0), draw(0.4))}
            {hair(1920.0, bottom, 1100.0, bottom, draw(0.8))}
            {hair(1770.0, y(1080.0), 1770.0, y(380.0), draw(1.2))}
            <g opacity={labels}>
                <rect x="186" y={top - 33.0} width="96" height="2" fill={RED} />
                <text x="310" y={top - 23.0} font-family={SERIF} font-weight="500" font-size="24" letter-spacing="7"
                    fill={TEXT} fill-opacity="0.8">"ILIOS"</text>
                <text x="1602" y={bottom - 22.0} text-anchor="end" font-family={SERIF} font-weight="500" font-size="28"
                    letter-spacing="6" fill={TEXT} fill-opacity="0.85">"イリオス"</text>
                <rect x="1628" y={bottom - 33.0} width="96" height="2" fill={RED} />
                <text transform="translate(112 540) rotate(90)" text-anchor="middle" font-family={SERIF} font-weight="500"
                    font-size="26" letter-spacing="6" fill={TEXT} fill-opacity="0.6">"Nenne"</text>
                <text transform="translate(1800 540) rotate(90)" text-anchor="middle" font-family={SERIF} font-weight="500"
                    font-size="26" letter-spacing="6" fill={TEXT} fill-opacity="0.6">"*Luna"</text>
            </g>
        </g>
    )
}

impl Video for IliosVideo<'_> {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(self.sections.iter().map(|s| s as &dyn Scene).collect::<Vec<_>>())
    }

    fn audio(&self) -> AudioMap<'_> {
        // The master peaks at +0.6 dBTP; 2 dB down keeps the encoded AAC under -1 dBTP.
        AudioMap::from([AudioTrack::new("audio.flac", Second(0.0)..Second(END)).gain_db(-2.0)])
    }

    fn render_frame<'b>(&'b self, frame: Frame, ctx: &FFramesContext<'b, '_>) -> Svgr<'b> {
        let t = frame.seconds();
        let audio = audio_features(&frame, ctx);
        let f = self.world.render(t, 1.0 / Self::FPS as f32, &audio);
        // Letterbox: the picture is wide in the verses and opens up on every chorus hit.
        let open = (ease_between(t, CHORUS_1 - 0.05, CHORUS_1 + 0.3) * (1.0 - ease_between(t, 91.0, 96.0)))
            .max(ease_between(t, CHORUS_2 - 0.05, CHORUS_2 + 0.3) * (1.0 - ease_between(t, 171.0, 175.0)))
            .max(ease_between(t, FINAL - 0.05, FINAL + 0.3) * (1.0 - ease_between(t, 248.0, 254.0)));
        let bars = 0.085 * (1.0 - open);
        let b = |k: usize| audio.bins.get(k).copied().unwrap_or(0.0);
        let mut uniforms = ShaderUniforms::new();
        for (g, name) in ["uB0", "uB1", "uB2", "uB3", "uB4", "uB5", "uB6", "uB7"].into_iter().enumerate() {
            uniforms = uniforms.float4(name, b(g * 4), b(g * 4 + 1), b(g * 4 + 2), b(g * 4 + 3));
        }
        let layer = self.post.draw(
            &frame,
            uniforms
                .image("uWorld", &f.world)
                .image("uText", &f.text)
                .float2("uPan", f.pan.x, f.pan.y)
                .float("uZoom", f.zoom)
                .float("uSpin", f.spin)
                .float("uHit", f.hit)
                .float("uBlue", f.blue)
                .float3("uRayColor", f.ray_color.x, f.ray_color.y, f.ray_color.z)
                .float("uRed", f.red_flash)
                .float("uLight", f.world_light)
                .float("uInvert", f.invert)
                .float("uRedFrame", f.red_frame)
                .float("uGlitch", f.glitch)
                .float("uSpeed", f.speed)
                .float("uSpeedDir", f.speed_dir)
                .float("uTwirl", f.twirl)
                .float("uBass", f.low)
                .float3("uCamR", f.cam_right.x, f.cam_right.y, f.cam_right.z)
                .float3("uCamU", f.cam_up.x, f.cam_up.y, f.cam_up.z)
                .float3("uCamF", f.cam_fwd.x, f.cam_fwd.y, f.cam_fwd.z)
                .float("uTanFov", f.tan_half_fov)
                .float("uTV", f.tv)
                .float("uCrt", f.crt)
                .float("uDome", f.dome)
                .float("uStop", f.stop)
                .float("uFlash", f.flash)
                .float("uReveal", f.reveal.0)
                .float("uRevealAmt", f.reveal.1)
                .float3("uSunDir", f.sun_dir.x, f.sun_dir.y, f.sun_dir.z)
                .float("uSunR", 0.11)
                .float("uSunAmt", f.sun_amt)
                .float("uEclipse", f.eclipse)
                .float3("uSunCol", 0.45, 0.62, 1.0)
                .float3("uNebA", f.neb_a.x, f.neb_a.y, f.neb_a.z)
                .float3("uNebB", f.neb_b.x, f.neb_b.y, f.neb_b.z)
                .float("uNeb", 0.42)
                .float("uShade", 0.0)
                .float3("uStarDir", f.star_dir.x, f.star_dir.y, f.star_dir.z)
                .float("uStarAmt", f.star_amt)
                .float3("uIrisDir", 0.0, 0.05, -1.0)
                .float("uIris", f.iris)
                .float("uVoice", f.voice)
                .float("uBars", bars)
                .float("uBeat", f.beat)
                .float("uToll", f.toll.0)
                .float("uTollAmt", f.toll.1)
                .float("uSwell", f.swell)
                .float("uSnare", f.snare)
                .float3("uDrop", f.drop.x, f.drop.y, f.drop.z)
                .float3("uKeyA", f.keys[0].x, f.keys[0].y, f.keys[0].z)
                .float3("uKeyB", f.keys[1].x, f.keys[1].y, f.keys[1].z)
                .float("uEcho", f.echo.0)
                .float("uEchoAmt", f.echo.1)
                .image("uAngel", &f.angel)
                .float("uAngelAmt", f.angel_amt)
                .float3("uGlowCol", f.glow_col.x, f.glow_col.y, f.glow_col.z)
                .float("uGlowAmt", f.glow_amt * (0.85 + 0.45 * audio.low))
                .float3("uAtmo", f.atmo.x, f.atmo.y, f.atmo.z)
                .float("uDay", f.eclipse)
                .float("uStars", 0.62 * (0.7 + 0.6 * audio.high))
                .float4("uComet", f.comet[0], f.comet[1], f.comet[2], f.comet[3])
                .float("uCometAmt", f.comet_amt),
        );

        // Small cover-style credits, after the title has gone.
        let labels = ease_out((t - 29.0) / 0.9) * (1.0 - ease_between(t, OUTRO - 1.0, OUTRO + 1.0));
        // End card on the first outro chime.
        let card = ease_out((t - CHIMES[0]) / 1.2) * (1.0 - ease_between(t, END - 3.0, END - 0.8));
        let card_rule = ease_out((t - CHIMES[0] - 0.4) / 1.4);
        let rule = ease_out((t - 29.3) / 0.8);
        // The frame steps aside for the big moments (the wormhole, the CRT, the end card).
        // The old frame of corner marks and rulers is gone; the picture carries its own look now.
        let hud_op = 0.0 * ease_out((t - 2.0) / 1.5) * (1.0 - f.twirl) * (1.0 - f.crt) * (1.0 - card) * (1.0 - 0.6 * f.flash)
            * (1.0 - ease_between(t, OUTRO - 1.0, OUTRO + 1.0));

        // The album cover's frame: it draws itself in under the title and leaves before the guitar's
        // strings cross the picture. It steps aside for the wormhole and the white flashes.
        let frame_op = (1.0 - f.twirl) * (1.0 - clamp01(f.flash)) * (1.0 - clamp01(1.3 * f.reveal.1)) * (1.0 - clamp01(f.dome)) * (1.0 - ease_between(t, 249.5, 252.5));

        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width={WIDTH} height={HEIGHT}>
                <image href={layer.href()} x="0" y="0" width="1920" height="1080" />
                {hud(t, hud_op)}
                {cover_frame(t, frame_op, bars * 1080.0)}
                <g opacity={labels * hud_op}>
                    <text x="1690" y="1012" text-anchor="end" font-family={SERIF} font-weight="500" font-size="30"
                        letter-spacing="6" fill={TEXT} fill-opacity="0.85">"イリオス"</text>
                    <rect x="1712" y="1001" width={0.5 + 96.0 * rule} height="2" fill={RED} />
                </g>
                <g opacity={card}>
                    <text x="960" y="548" text-anchor="middle" font-family={SERIF} font-weight="800" font-size="64"
                        letter-spacing="22" fill={TEXT}>"イリオス"</text>
                    <rect x={960.0 - 60.0 * card_rule} y="584" width={0.5 + 120.0 * card_rule} height="2" fill={RED} />
                    <text x="960" y="652" text-anchor="middle" font-family={SERIF} font-weight="500" font-size="40"
                        letter-spacing="10" fill={TEXT} fill-opacity="0.8">"Nenne  /  *Luna"</text>
                </g>
                {ctx.render_scenes(&frame)}
            </svg>
        )
    }
}
