#!/usr/bin/env bash
# Render the video and encode a copy to upload to YouTube, X/Twitter and the like.
#
#   ./export.sh            build, render the master, then encode the upload copy
#   ./export.sh --encode   only encode the upload copy from an existing master
#
# Writes ilios-x.mp4 too: the first 2:20 for X, see the end.
# Writes ilios-master.mp4 (about 1.4 GB, 40 Mbps) and ilios-1080p.mp4 (1080p30 H.264 High,
# yuv420p, AAC-LC 320 kbps 48 kHz, faststart). The video is encoded on the GPU (VideoToolbox)
# at an average 20 Mbps (about 720 MB): the film grain and scanlines blocked up at 12 Mbps.
# For YouTube, upload the master itself; this copy is for sites that cap the file size.
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
