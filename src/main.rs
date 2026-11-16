use fframes::{CombinedMediaProvider, EncoderOptions, MediaDirectory, MediaProvider, RenderOptions, StaticMediaProvider, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use ilios::{HEIGHT, IliosMedia, IliosVideo, WIDTH};
use std::process::ExitCode;

fn main() -> ExitCode {
    let static_media = IliosMedia::prepare().expect("media");
    // The song stays stereo and outside the binary.
    let assets = MediaDirectory::read_folder(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).expect("assets");
    let audio = assets.process_media_source().expect("audio");
    let media = CombinedMediaProvider::from([&static_media as &dyn MediaProvider, &audio]);

    let video = IliosVideo::new(&static_media);
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            default_font: "Shippori Mincho",
            video_encoder_options: EncoderOptions {
                // Apple's hardware encoder: frames go from the Metal surface straight to the
                // media engine, so the CPU stays free (libx264 was ~100% of the CPU time).
                preferred_encoder: Some("h264_videotoolbox"),
                codec_params: Some(&[("b", "40M"), ("maxrate", "60M"), ("profile", "high")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .backend(
        SkiaFFramesRenderer::new_metal(
            &gpu,
            SkiaPipelineConfig {
                concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
                ..Default::default()
            },
        )
        .expect("skia renderer"),
    )
    .preview(fframes_native_player::cli_preview)
    .default_output("ilios-proto.mp4")
    .run()
}
