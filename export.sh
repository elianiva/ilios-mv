#!/usr/bin/env bash
# Render the video and encode a copy to upload to YouTube, X/Twitter and the like.
#
#   ./export.sh            build, render the master, then encode the upload copy
#   ./export.sh --encode   only encode the upload copy from an existing master
#
# Also writes ilios-youtube.mp4 (1440p, for YouTube) and ilios-x.mp4 (the first 2:20, for X).
# Writes ilios-master.mp4 (about 1.4 GB, 40 Mbps) and ilios-1080p.mp4 (1080p30 H.264 High,
# yuv420p, AAC-LC 320 kbps 48 kHz, faststart). The video is encoded on the GPU (VideoToolbox)
# at an average 20 Mbps (about 720 MB): the film grain and scanlines blocked up at 12 Mbps.
# For YouTube, upload ilios-youtube.mp4 (below); this copy is for sites that cap the file size.
set -euo pipefail
cd "$(dirname "$0")"

MASTER=ilios-master.mp4
OUT=ilios-1080p.mp4

if [[ "${1:-}" != "--encode" ]]; then
    # The x264 that the renderer links against. Homebrew's is missing on this Mac.
    : "${LIBRARY_PATH:=/nix/store/ah560w1rplhnj1qxhjvpzz8f1yvmhdff-x264-0-unstable-2025-09-10-lib/lib}"
    export LIBRARY_PATH
    cargo build --release
    ./target/release/ilios render -o "$MASTER"
fi

[[ -f "$MASTER" ]] || { echo "no $MASTER: run without --encode first" >&2; exit 1; }
duration=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$MASTER")

# The audio comes from the FLAC, not from the master's AAC, so it is encoded only once. It gets
# the same -2 dB the render applies.
ffmpeg -hide_banner -y \
    -i "$MASTER" -i assets/audio.flac \
    -map 0:v:0 -map 1:a:0 -t "$duration" \
    -c:v h264_videotoolbox -b:v 20M -maxrate 28M -bufsize 56M -allow_sw 0 \
    -profile:v high -pix_fmt yuv420p -g 60 \
    -color_primaries bt709 -color_trc bt709 -colorspace bt709 \
    -af "volume=-2dB" -c:a aac_at -b:a 320k -ar 48000 -ac 2 \
    -movflags +faststart \
    "$OUT"

ls -lh "$OUT"

# YouTube: 1440p, upscaled from the master. YouTube encodes 1440p uploads with VP9/AV1 at every
# size and plays them in HD by default, where a 1080p upload often gets only H.264 at a low
# bitrate. Testing in 2026 measured about 97.3 VMAF for what YouTube serves from a 1080p upload
# and 99.95 from a 1440p one; 4K adds nothing over 1440p. The master is upscaled rather than
# rendered at 1440p because the shader's glow and grain are sized in pixels and would change.
# HEVC Main10 at 60 Mbps, well above YouTube's 24 Mbps for 1440p30, scored best as an upload.
YT_OUT=ilios-youtube.mp4
ffmpeg -hide_banner -y \
    -i "$MASTER" -i assets/audio.flac \
    -map 0:v:0 -map 1:a:0 -t "$duration" \
    -vf "scale=2560:1440:flags=lanczos,format=p010le" \
    -c:v hevc_videotoolbox -profile:v main10 -b:v 60M -maxrate 80M -bufsize 160M -allow_sw 0 -tag:v hvc1 -g 15 \
    -color_primaries bt709 -color_trc bt709 -colorspace bt709 \
    -af "volume=-2dB" -c:a aac_at -b:a 384k -ar 48000 -ac 2 \
    -movflags +faststart \
    "$YT_OUT"

ls -lh "$YT_OUT"

# X without Premium takes 2:20 at most: the first 2:20, fading to black and silence over the
# last 6 s. Once the picture is black, a note says to go watch the full one on YouTube. It is
# encoded at 33 Mbps, about as much as fits in X's 512 MB for 2:20 (about 470 MB).
X_OUT=ilios-x.mp4
X_LEN=139.9
FADE_AT=133.9
notes=$(mktemp -d)
printf '%s' "watch the full one on youtube ;)" > "$notes/a.txt"
printf '%s' "i don't have x premium so i can't upload the full one here lol" > "$notes/b.txt"
note="fontcolor=0xe4d2d7:x=(w-text_w)/2:alpha='clip((t-$FADE_AT-3.5)/1.2,0,1)'"
ffmpeg -hide_banner -y \
    -i "$MASTER" -i assets/audio.flac \
    -map 0:v:0 -map 1:a:0 -t "$X_LEN" \
    -vf "fade=t=out:st=$FADE_AT:d=4.0,\
drawtext=fontfile=media/ShipporiMincho-ExtraBold.ttf:textfile=$notes/a.txt:fontsize=58:y=h/2-70:$note,\
drawtext=fontfile=media/ShipporiMincho-Medium.ttf:textfile=$notes/b.txt:fontsize=30:y=h/2+20:$note" \
    -c:v h264_videotoolbox -b:v 33M -maxrate 40M -bufsize 80M -allow_sw 0 \
    -profile:v high -pix_fmt yuv420p -g 60 \
    -color_primaries bt709 -color_trc bt709 -colorspace bt709 \
    -af "volume=-2dB,afade=t=out:st=$FADE_AT:d=5.6" -c:a aac_at -b:a 320k -ar 48000 -ac 2 \
    -movflags +faststart \
    "$X_OUT"
rm -r "$notes"

ls -lh "$X_OUT"
