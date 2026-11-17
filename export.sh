#!/usr/bin/env bash
# Render the video and encode a copy to upload to YouTube, X/Twitter and the like.
#
#   ./export.sh            build, render the master, then encode the upload copy
#   ./export.sh --encode   only encode the upload copy from an existing master
#
# Writes ilios-master.mp4 (about 1.4 GB, 40 Mbps) and ilios-1080p.mp4 (1080p30 H.264 High,
# yuv420p, AAC-LC 320 kbps 48 kHz, faststart). The video is encoded on the GPU (VideoToolbox)
# at an average 7 Mbps, about the 8 Mbps YouTube recommends for 1080p30, so the copy is about
# 260 MB and well under X's 512 MB limit.
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
    -c:v h264_videotoolbox -b:v 7M -maxrate 10M -bufsize 20M -allow_sw 0 \
    -profile:v high -pix_fmt yuv420p -g 60 \
    -color_primaries bt709 -color_trc bt709 -colorspace bt709 \
    -af "volume=-2dB" -c:a aac_at -b:a 320k -ar 48000 -ac 2 \
    -movflags +faststart \
    "$OUT"

ls -lh "$OUT"
