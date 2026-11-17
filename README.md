# イリオス (Ilios) fan MV

A fan-made lyric video for [イリオス](https://youtu.be/ZmusF-VLHsI) by ねんね (vocals) and
*Luna (music). The whole video is code: a Rust program built on
[fframes](https://github.com/dmtrKovalenko/fframes) that draws every frame with Skia on Metal
and a 3D scene renderer. It renders 1920×1080 at 30 fps.

## Requirements

- macOS on Apple silicon. The renderer uses Metal and the encoder uses VideoToolbox.
- Rust (edition 2024) and ffmpeg.
- libx264, which fframes links against. Point `LIBRARY_PATH` at its `lib` directory if the
  linker can't find it.
- The song. The audio is not in this repository. Download it to `assets/audio.flac`:

  ```sh
  mkdir -p assets
  yt-dlp -x --audio-format flac -o assets/audio.flac "https://youtu.be/ZmusF-VLHsI"
  ```

  All the timing in `src/timing.rs` and `lyrics/ilios.lrc` is measured against this upload.

## Layout

| Path | Contents |
| --- | --- |
| `src/world.rs` | The 3D world: lyric lines, camera path, the souls, every per-line effect |
| `src/graphics.rs` | Magic circles, stars, rings, and the audio-reactive parts |
| `src/timing.rs` | Song structure and hand- and FFT-measured cues (beats, guitar, keys) |
| `src/lib.rs` | The fframes video: scenes, overlays, the cover frame, the thumbnail switch |
| `src/shaders/post.sksl` | The post shader: sky, nebula, flashes, scanlines, the film look |
| `lyrics/ilios.lrc` | Word-timed lyrics |
| `media/` | Shippori Mincho fonts (SIL OFL) |
| `export.sh` | Renders the master and encodes the upload copies |
| `thumbnail.py` | Grades the thumbnail frame |

## Work on it

```sh
cargo run --release -- preview 1:30 --paused   # real-time window with sound (space, h/l, j/k, q)
cargo run --release -- frame 40s               # one PNG into frames/
cargo run --release -- strip 35s..41s -n 12    # contact sheet, writes strip.png
cargo run --release -- render 35s..41s --draft # a part of the video, fast
```

Times accept frames (`120`), seconds (`3.2s`), `m:ss`, percentages (`50%`), and scene names.

## Export

```sh
./export.sh            # build, render ilios-master.mp4, then encode the copies
./export.sh --encode   # encode the copies from an existing master
```

It writes three files, all encoded on the GPU:

- `ilios-youtube.mp4`: 4K HEVC, the master upscaled. YouTube gives uploads above 1080p better
  codecs and more bits at every size.
- `ilios-1080p.mp4`: 1080p H.264 for other sites.
- `ilios-x.mp4`: the first 2:20 with a fade and a note, for X without Premium.

The thumbnail is its own frame of the title card under a still camera:

```sh
ILIOS_THUMBNAIL=1 ./target/release/ilios --scale 1.5 frame 0s
python3 thumbnail.py frames/0s.png thumbnail
```

## Credits

イリオス by ねんね and *Luna. This is an unofficial fan work, and the song and lyrics belong
to their creators.
