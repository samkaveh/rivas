#!/usr/bin/env bash
# Record the README demo GIF (rivas_demo_short.gif) on macOS.
#
# Opens a dedicated Kitty window running rivas, records that window with
# ffmpeg while a scripted tour is typed into it through Kitty's remote
# control, then encodes an optimized GIF.
#
# Requirements: macOS, kitty, ffmpeg; gifsicle is optional (smaller output).
# The terminal you run this from needs Screen Recording permission
# (System Settings › Privacy & Security › Screen Recording).
# Keep the Kitty window visible and don't touch the keyboard or mouse while
# it records.
#
# Usage: scripts/make-demo-gif.sh [output.gif]
#
# Environment overrides:
#   DEMO_FILE   Markdown file to tour   (default: examples/all-rendering-cases.md)
#   COLS, ROWS  terminal size in cells  (default: 110 x 34)
#   FONT_SIZE   Kitty font size, pt     (default: 13)
#   GIF_WIDTH   output width, px        (default: 960)
#   GIF_FPS     output frame rate       (default: 10)
#   KEEP_RAW=1  keep the raw screen recording (.mkv) next to the GIF
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/rivas_demo_short.gif}"
DEMO_FILE="${DEMO_FILE:-examples/all-rendering-cases.md}"
COLS="${COLS:-110}"
ROWS="${ROWS:-34}"
FONT_SIZE="${FONT_SIZE:-13}"
GIF_WIDTH="${GIF_WIDTH:-960}"
GIF_FPS="${GIF_FPS:-10}"

die() { echo "error: $*" >&2; exit 1; }
log() { echo "▸ $*"; }

[[ "$(uname)" == "Darwin" ]] || die "this script records the screen with macOS APIs"
command -v kitty >/dev/null || die "kitty is required (brew install --cask kitty)"
command -v ffmpeg >/dev/null || die "ffmpeg is required (brew install ffmpeg)"
[[ -f "$ROOT/$DEMO_FILE" ]] || die "demo file not found: $DEMO_FILE"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/rivas-demo.XXXXXX")"
SOCK="$WORK/kitty.sock"
# Matroska survives an interrupted recording, unlike .mov.
RAW="$WORK/demo.mkv"
KITTY_PID=""
FFMPEG_PID=""

cleanup() {
    [[ -n "$FFMPEG_PID" ]] && kill -KILL "$FFMPEG_PID" 2>/dev/null || true
    [[ -n "$KITTY_PID" ]] && kill "$KITTY_PID" 2>/dev/null || true
    if [[ "${KEEP_RAW:-0}" == "1" && -f "$RAW" ]]; then
        cp "$RAW" "${OUT%.gif}.mkv"
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT

# Screen capture needs the Screen Recording permission; without it ffmpeg
# just waits forever, so check up front with a one-pixel screenshot.
if ! screencapture -x -R 0,0,1,1 "$WORK/probe.png" 2>/dev/null; then
    die "no Screen Recording permission. Allow your terminal app under
       System Settings › Privacy & Security › Screen Recording, restart it,
       and run this script again."
fi

# ── Build ────────────────────────────────────────────────────────────────────
log "Building rivas (release)"
cargo build --release --quiet --manifest-path "$ROOT/Cargo.toml" 2>/dev/null \
    || cargo build --release --manifest-path "$ROOT/Cargo.toml"
RIVAS="$ROOT/target/release/rivas"

# ── Launch Kitty ─────────────────────────────────────────────────────────────
# A clean, self-contained config so the recording doesn't depend on the
# user's kitty.conf. The background matches rivas' default theme so the
# window padding blends in.
log "Opening Kitty (${COLS}x${ROWS})"
kitty --config NONE \
    --listen-on "unix:$SOCK" \
    -o allow_remote_control=socket-only \
    -o remember_window_size=no \
    -o initial_window_width="${COLS}c" \
    -o initial_window_height="${ROWS}c" \
    -o font_size="$FONT_SIZE" \
    -o window_padding_width=6 \
    -o background='#1a1b26' \
    -o cursor_blink_interval=0 \
    -o confirm_os_window_close=0 \
    -o macos_quit_when_last_window_closed=yes \
    --title "rivas" \
    --directory "$ROOT" \
    "$RIVAS" "$DEMO_FILE" &
KITTY_PID=$!

for _ in $(seq 50); do
    [[ -S "$SOCK" ]] && break
    sleep 0.2
done
[[ -S "$SOCK" ]] || die "kitty did not open its remote-control socket"
sleep 2 # let the window settle and the first graphics render

# ── Locate the window ────────────────────────────────────────────────────────
# Window bounds (in points) of the Kitty instance we started, and the main
# display's pixel scale, from CoreGraphics via JavaScript for Automation.
read -r WX WY WW WH SCALE < <(osascript -l JavaScript - "$KITTY_PID" <<'JXA'
ObjC.import('CoreGraphics');
ObjC.import('AppKit');
function run(argv) {
    const pid = parseInt(argv[0], 10);
    const list = ObjC.castRefToObject(
        $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0));
    let best = null;
    for (let i = 0; i < list.count; i++) {
        const w = list.objectAtIndex(i);
        if (ObjC.unwrap(w.objectForKey('kCGWindowOwnerPID')) !== pid) continue;
        if (ObjC.unwrap(w.objectForKey('kCGWindowLayer')) !== 0) continue;
        const b = ObjC.deepUnwrap(w.objectForKey('kCGWindowBounds'));
        if (!best || b.Width * b.Height > best.Width * best.Height) best = b;
    }
    if (!best) return '';
    const scale = $.NSScreen.mainScreen.backingScaleFactor;
    return [best.X, best.Y, best.Width, best.Height, scale].join(' ');
}
JXA
)
[[ -n "${WX:-}" ]] || die "could not find the Kitty window on screen"
log "Window at ${WX},${WY} ${WW}x${WH} (scale ${SCALE})"

# Crop rectangle in screen pixels (even sizes for the encoder).
px() { awk -v v="$1" -v s="$SCALE" 'BEGIN { printf "%d", v * s }'; }
even() { echo $(( $1 / 2 * 2 )); }
CX=$(px "$WX"); CY=$(px "$WY")
CW=$(even "$(px "$WW")"); CH=$(even "$(px "$WH")")

# ffmpeg's avfoundation index of the main screen ("Capture screen 0").
# (Listing devices always exits non-zero, hence `|| true`.)
SCREEN_IDX=$(ffmpeg -hide_banner -f avfoundation -list_devices true -i "" 2>&1 \
    | sed -n 's/.*\[\([0-9]*\)\] Capture screen 0.*/\1/p' | head -1 || true)
[[ -n "$SCREEN_IDX" ]] || die "ffmpeg can't see the screen (Screen Recording permission?)"

# ── Tour helpers ─────────────────────────────────────────────────────────────
# `send-text` understands escapes: \x1b is Esc, \x04 is Ctrl-d, \r is Enter.
send() { kitty @ --to "unix:$SOCK" send-text -- "$1"; }
key() { send "$1"; sleep "${2:-0.35}"; }
typed() { # type text at a human pace
    local s="$1" i
    for ((i = 0; i < ${#s}; i++)); do send "${s:i:1}"; sleep 0.07; done
}
pause() { sleep "$1"; }

# ── Record ───────────────────────────────────────────────────────────────────
log "Recording — hands off the keyboard and mouse"
# ffmpeg's chatter (pixel-format notes, objc warnings) goes to a log that is
# shown only if recording fails; `-progress` reports the live frame count.
FFLOG="$WORK/ffmpeg.log"
PROGRESS="$WORK/progress"
ffmpeg -hide_banner -loglevel warning -nostats -y \
    -f avfoundation -capture_cursor 0 -pixel_format uyvy422 -framerate 30 \
    -i "${SCREEN_IDX}:none" \
    -vf "crop=${CW}:${CH}:${CX}:${CY}" \
    -c:v libx264 -preset ultrafast -crf 12 -pix_fmt yuv420p \
    -progress "$PROGRESS" \
    "$RAW" </dev/null 2>"$FFLOG" &
FFMPEG_PID=$!

# Wait for the first captured frames. (The output file itself can't be
# watched: ffmpeg buffers it, so it stays empty for a while.)
recording_started() {
    [[ -f "$PROGRESS" ]] && grep -q '^frame=[1-9]' "$PROGRESS"
}
for _ in $(seq 75); do
    recording_started && break
    kill -0 "$FFMPEG_PID" 2>/dev/null || break
    sleep 0.2
done
if ! recording_started; then
    sed 's/^/  ffmpeg: /' "$FFLOG" >&2
    die "the screen recording didn't start (see the ffmpeg output above)"
fi
pause 2

# 1. The rendered document: scroll through headings, lists, code, tables,
#    images, diagrams and math.
for _ in $(seq 11); do key $'\x04' 1.6; done
key g 0.1; key g 1.2

# 2. Edit in place: jump to a paragraph, append text, leave Insert mode.
key / 0.4; typed "fixture"; key $'\r' 1.0
key A 0.6; typed " Edit right where you read."; pause 0.8
key $'\x1b' 1.6
key u 1.2

# 3. The Space menu: discover actions without memorizing keys.
key ' ' 2.0
key e 1.6 # explorer opens and takes focus
key j 0.5; key j 0.5; key k 0.8
key $'\x0c' 0.8 # Ctrl-l: back to the document, explorer stays open
key j 0.3; key j 0.3; key j 1.0
key '\\' 0.8; key '\\' 1.0 # focus the explorer, then hide it

# 4. Themes, from the UI group.
key ' ' 1.0; key u 1.6; key t 2.2
key ' ' 0.6; key u 0.6; key t 2.2

# 5. The full keyboard reference.
key ' ' 0.8; key '?' 3.5
key $'\x1b' 1.5

# SIGTERM makes ffmpeg finish the file. (SIGINT would be ignored: background
# jobs of a non-interactive shell start with SIGINT ignored.)
stop_recording() {
    kill -TERM "$FFMPEG_PID" 2>/dev/null || return 0
    for _ in $(seq 50); do
        kill -0 "$FFMPEG_PID" 2>/dev/null || break
        sleep 0.1
    done
    kill -KILL "$FFMPEG_PID" 2>/dev/null || true
    wait "$FFMPEG_PID" 2>/dev/null || true
    FFMPEG_PID=""
}
stop_recording
key ' ' 0.3; key q 0.3; key '!' 0.3

# ── Encode ───────────────────────────────────────────────────────────────────
log "Encoding GIF (${GIF_WIDTH}px, ${GIF_FPS} fps)"
ffmpeg -hide_banner -loglevel error -y -i "$RAW" \
    -vf "fps=${GIF_FPS},scale=${GIF_WIDTH}:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=192:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
    "$WORK/demo.gif"
if command -v gifsicle >/dev/null; then
    gifsicle -O3 --lossy=30 "$WORK/demo.gif" -o "$OUT"
else
    cp "$WORK/demo.gif" "$OUT"
fi
log "Wrote $OUT ($(du -h "$OUT" | cut -f1 | tr -d ' '))"
