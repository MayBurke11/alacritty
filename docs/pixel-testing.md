# Pixel-Analysis Testing for Rendering

> How to test layout/rendering without seeing the screen

## Problem

Traditional `cargo build --release` takes ~60s. Then launch, resize, visually inspect. Per iteration: 2 minutes.

## Solution

**Debug build (6s) + ImageMagick pixel sampling** — verify layout in code.

## Workflow

### 1. Build debug (6s)
```bash
cargo build 2>&1 | tail -1
# Finished `dev` profile in 6.29s
```

### 2. Launch + resize + screenshot (1s)
```bash
./target/debug/alacritty --config-file /tmp/menu.toml &
sleep 2
WID=$(xdotool search --class Alacritty | head -1)
xdotool windowsize $WID 800 400  # test size
sleep 0.4
import -window $WID /tmp/test.png
```

### 3. Analyze pixels (0.5s)

**Check bar position:**
```bash
# Scan vertical column at mid-x, bottom 30 pixels
for y in 370 380 390 398; do
  convert /tmp/test.png -crop 1x1+400+$y txt:- | tail -1 | awk '{print $3}'
done
# #1E1E2E = terminal background
# #181824 = bar background (primary * 0.8)
# Light gray (#DCDCDC) = text pixels
```

**Check text visibility:**
```bash
# Full-width scan at bar Y position
convert /tmp/test.png -crop 800x1+0+382 -resize 80x1! txt:- | awk '{print $3}' | grep -vc '#181824'
# > 0 means text is visible (non-background pixels exist)
```

**Test all sizes:**
```bash
for h in 300 400 500 600 800; do
  xdotool windowsize $WID 800 $h
  sleep 0.4; import -window $WID /tmp/h$h.png
  # Check text at y = h-15 (near bottom)
  TEXT=$(convert /tmp/h$h.png -crop 800x1+0+$(($h-15)) -resize 80x1! txt:- | awk '{print $3}' | grep -vc '#181824')
  echo "$h: text_pixels=$TEXT"
done
```

## Key colors

| Color | Hex | Meaning |
|---|---|---|
| `#1E1E2E` | focus_nova bg | Terminal background |
| `#181824` | bg * 0.8 | Bar background |
| `#DCDCDC` | primary.foreground | Text |
| `#808089` | gray | Active tab bg |

## Result

10 iterations in 2 minutes instead of 20 minutes.
Found 3 bugs (line calculation, submenu position, viewport clipping) purely through pixel data.
