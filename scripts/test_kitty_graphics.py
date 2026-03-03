#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.12"
# ///
"""
Manual test script for kitty graphics protocol support.

Usage:
    # First, build and launch the alacritty fork:
    cd kitty/alacritty
    cargo build && ./target/debug/alacritty

    # Then, inside that alacritty window, run:
    uv run scripts/test_kitty_graphics.py

    # Or run individual tests:
    python3 scripts/test_kitty_graphics.py query
    python3 scripts/test_kitty_graphics.py png
    python3 scripts/test_kitty_graphics.py rgba
    python3 scripts/test_kitty_graphics.py rgb
    python3 scripts/test_kitty_graphics.py chunked
    python3 scripts/test_kitty_graphics.py delete
    python3 scripts/test_kitty_graphics.py zlib
    python3 scripts/test_kitty_graphics.py file <path_to_image.png>

See: https://sw.kovidgoyal.net/kitty/graphics-protocol/
"""

import base64
import io
import os
import select
import struct
import sys
import termios
import tty
import zlib


def drain_responses(timeout: float = 0.1) -> list[str]:
    """Read and discard any pending APC responses from stdin.

    When we send kitty graphics commands, the terminal writes responses
    back to the PTY (e.g. ESC _Gi=1;OK ESC \\). If we don't consume
    them, they leak into the shell prompt as garbage text.

    Returns the raw response strings for debugging.
    """
    responses = []
    fd = sys.stdin.fileno()

    # Save terminal state and switch to raw mode so we can read
    # without waiting for Enter.
    old_settings = termios.tcgetattr(fd)
    try:
        tty.setraw(fd)
        buf = bytearray()
        while True:
            ready, _, _ = select.select([fd], [], [], timeout)
            if not ready:
                break
            chunk = os.read(fd, 4096)
            if not chunk:
                break
            buf.extend(chunk)
        if buf:
            responses.append(buf.decode("utf-8", errors="replace"))
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old_settings)

    return responses


def write_apc(payload: str, expect_response: bool = True) -> list[str]:
    """Write a kitty graphics APC sequence to stdout.

    If expect_response is True, drains the PTY response so it doesn't
    leak into the shell prompt.
    """
    sys.stdout.write(f"\033_G{payload}\033\\")
    sys.stdout.flush()
    if expect_response:
        return drain_responses()
    return []


def write_chunked(key_values: str, data: bytes, chunk_size: int = 4096) -> list[str]:
    """Write image data in chunks per the kitty protocol."""
    encoded = base64.standard_b64encode(data).decode("ascii")
    chunks = [encoded[i : i + chunk_size] for i in range(0, len(encoded), chunk_size)]

    if not chunks:
        return write_apc(f"{key_values},m=0;")

    for i, chunk in enumerate(chunks):
        is_last = i == len(chunks) - 1
        m = 0 if is_last else 1
        if i == 0:
            # Intermediate chunks don't get responses; only drain on last.
            write_apc(f"{key_values},m={m};{chunk}", expect_response=is_last)
        else:
            write_apc(f"m={m};{chunk}", expect_response=is_last)
    return []


def make_solid_rgba(width: int, height: int, r: int, g: int, b: int, a: int = 255) -> bytes:
    """Create a solid-color RGBA image as raw bytes."""
    pixel = bytes([r, g, b, a])
    return pixel * (width * height)


def make_gradient_rgba(width: int, height: int) -> bytes:
    """Create an RGBA gradient image (red→blue horizontally, green vertically)."""
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            r = int(255 * x / max(width - 1, 1))
            g = int(255 * y / max(height - 1, 1))
            b = 255 - r
            pixels.extend([r, g, b, 255])
    return bytes(pixels)


def make_checkerboard_rgba(width: int, height: int, sq: int = 8) -> bytes:
    """Create an RGBA checkerboard pattern."""
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            if ((x // sq) + (y // sq)) % 2 == 0:
                pixels.extend([200, 200, 200, 255])
            else:
                pixels.extend([50, 50, 50, 255])
    return bytes(pixels)


def make_test_png(width: int, height: int) -> bytes:
    """Create a minimal PNG image in memory (no external deps)."""
    # We'll build a valid PNG by hand: IHDR, IDAT, IEND.
    # Using uncompressed deflate blocks for simplicity.

    def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
        chunk = chunk_type + data
        crc = struct.pack(">I", zlib.crc32(chunk) & 0xFFFFFFFF)
        return struct.pack(">I", len(data)) + chunk + crc

    # PNG signature
    sig = b"\x89PNG\r\n\x1a\n"

    # IHDR: width, height, bit_depth=8, color_type=2 (RGB), compression=0, filter=0, interlace=0
    ihdr_data = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    ihdr = png_chunk(b"IHDR", ihdr_data)

    # Image data: each row has a filter byte (0=None) followed by RGB pixels
    raw_rows = bytearray()
    for y in range(height):
        raw_rows.append(0)  # filter: None
        for x in range(width):
            r = int(255 * x / max(width - 1, 1))
            g = int(255 * y / max(height - 1, 1))
            b = 128
            raw_rows.extend([r, g, b])

    compressed = zlib.compress(bytes(raw_rows))
    idat = png_chunk(b"IDAT", compressed)

    iend = png_chunk(b"IEND", b"")

    return sig + ihdr + idat + iend


# ── Test Functions ──────────────────────────────────────────────────────


def test_query():
    """Test 1: Query protocol support (a=q).

    Expected: Terminal should respond with ESC _Gi=1;OK ESC \\
    You can check with: cat -v  (then paste the query, you should see a response)
    """
    print("═" * 60)
    print("TEST: Query protocol support (a=q)")
    print("  Sending query... (response goes to PTY input)")
    print("  Expected response: \\e_Gi=1;OK\\e\\\\")
    print("═" * 60)

    # Query with a tiny 1x1 PNG to test format support.
    png_data = make_test_png(1, 1)
    encoded = base64.standard_b64encode(png_data).decode("ascii")
    responses = write_apc(f"a=q,i=1,f=100;{encoded}")
    if responses:
        raw = responses[0]
        if "OK" in raw:
            print(f"  ✓ Query sent — got OK response")
        else:
            print(f"  ✗ Query sent — unexpected response: {raw!r}")
    else:
        print("  ⚠ Query sent — no response received (timeout)")
    print()


def test_rgba():
    """Test 2: Display a raw RGBA image (f=32, a=T)."""
    print("═" * 60)
    print("TEST: Raw RGBA image (32x32 gradient)")
    print("═" * 60)

    width, height = 32, 32
    pixels = make_gradient_rgba(width, height)
    responses = write_chunked(f"a=T,f=32,s={width},v={height},i=10", pixels)
    print("  ✓ Sent 32x32 RGBA gradient (image id=10)")
    print("  You should see a small colorful square above.\n")


def test_rgb():
    """Test 3: Display a raw RGB image (f=24, a=T)."""
    print("═" * 60)
    print("TEST: Raw RGB image (24x24 solid red)")
    print("═" * 60)

    width, height = 24, 24
    # RGB = 3 bytes per pixel
    pixel = bytes([255, 0, 0])
    pixels = pixel * (width * height)
    responses = write_chunked(f"a=T,f=24,s={width},v={height},i=11", pixels)
    print("  ✓ Sent 24x24 solid red RGB (image id=11)")
    print("  You should see a small red square above.\n")


def test_png():
    """Test 4: Display a PNG image (f=100, a=T)."""
    print("═" * 60)
    print("TEST: PNG image (64x32 gradient)")
    print("═" * 60)

    png_data = make_test_png(64, 32)
    responses = write_chunked("a=T,f=100,i=12", png_data)
    print(f"  ✓ Sent 64x32 PNG ({len(png_data)} bytes, image id=12)")
    print("  You should see a gradient rectangle above.\n")


def test_chunked():
    """Test 5: Chunked transfer with small chunk size."""
    print("═" * 60)
    print("TEST: Chunked transfer (48x48 checkerboard, 256-byte chunks)")
    print("═" * 60)

    width, height = 48, 48
    pixels = make_checkerboard_rgba(width, height)
    # Use very small chunks to exercise the chunked code path.
    responses = write_chunked(f"a=T,f=32,s={width},v={height},i=13", pixels, chunk_size=256)
    print(f"  ✓ Sent 48x48 checkerboard in 256-byte chunks (image id=13)")
    print("  You should see a checkerboard pattern above.\n")


def test_zlib():
    """Test 6: Zlib-compressed RGBA image (o=z)."""
    print("═" * 60)
    print("TEST: Zlib-compressed RGBA image (32x32 solid blue)")
    print("═" * 60)

    width, height = 32, 32
    pixels = make_solid_rgba(width, height, 0, 0, 255)
    compressed = zlib.compress(pixels)
    responses = write_chunked(f"a=T,f=32,o=z,s={width},v={height},i=14", compressed)
    print(f"  ✓ Sent 32x32 blue (compressed {len(pixels)}→{len(compressed)} bytes, image id=14)")
    print("  You should see a solid blue square above.\n")


def test_transmit_then_display():
    """Test 7: Transmit (a=t) then display (a=p) separately."""
    print("═" * 60)
    print("TEST: Separate transmit (a=t) then display (a=p)")
    print("═" * 60)

    width, height = 32, 32
    pixels = make_solid_rgba(width, height, 0, 200, 0)
    responses = write_chunked(f"a=t,f=32,s={width},v={height},i=15", pixels)
    print("  ✓ Transmitted 32x32 green (image id=15) — no display yet")

    # Now display it.
    responses = write_apc("a=p,i=15")
    print("  ✓ Displayed image id=15")
    print("  You should see a solid green square above.\n")


def test_delete():
    """Test 8: Delete operations."""
    print("═" * 60)
    print("TEST: Delete images")
    print("═" * 60)

    # Place a small image.
    width, height = 16, 16
    pixels = make_solid_rgba(width, height, 255, 255, 0)
    responses = write_chunked(f"a=T,f=32,s={width},v={height},i=20", pixels)
    print("  ✓ Placed 16x16 yellow square (image id=20)")

    # Delete by ID — delete commands don't produce responses.
    write_apc("a=d,d=i,i=20", expect_response=False)
    print("  ✓ Deleted image id=20")
    print("  The yellow square may still be visible (cell content cleared on next redraw).\n")


def test_file(path: str):
    """Test 9: Display a PNG file from disk."""
    print("═" * 60)
    print(f"TEST: Display PNG file: {path}")
    print("═" * 60)

    if not os.path.exists(path):
        print(f"  ✗ File not found: {path}")
        return

    with open(path, "rb") as f:
        data = f.read()

    responses = write_chunked("a=T,f=100,i=30", data)
    print(f"  ✓ Sent {len(data)} bytes from {path} (image id=30)")
    print("  You should see the image above.\n")


def test_quiet_modes():
    """Test 10: Quiet mode (q=1 suppresses OK, q=2 suppresses all)."""
    print("═" * 60)
    print("TEST: Quiet modes (q=0, q=1, q=2)")
    print("═" * 60)

    png_data = make_test_png(1, 1)
    encoded = base64.standard_b64encode(png_data).decode("ascii")

    r0 = write_apc(f"a=q,q=0,i=40,f=100;{encoded}")
    got0 = "OK" in r0[0] if r0 else False
    print(f"  {'✓' if got0 else '⚠'} q=0 query — {'got OK' if got0 else 'no response'}")

    r1 = write_apc(f"a=q,q=1,i=41,f=100;{encoded}")
    got1 = len(r1) == 0 or not any("OK" in r for r in r1)
    print(f"  {'✓' if got1 else '✗'} q=1 query — {'OK suppressed' if got1 else 'got unexpected response'}")

    r2 = write_apc(f"a=q,q=2,i=42,f=100;{encoded}")
    got2 = len(r2) == 0 or not any("OK" in r for r in r2)
    print(f"  {'✓' if got2 else '✗'} q=2 query — {'all suppressed' if got2 else 'got unexpected response'}")
    print()


def test_cursor_no_move():
    """Test 11: C=1 (don't move cursor after placement)."""
    print("═" * 60)
    print("TEST: Cursor movement control (C=1)")
    print("═" * 60)

    width, height = 16, 16
    pixels = make_solid_rgba(width, height, 200, 100, 50)
    encoded = base64.standard_b64encode(pixels).decode("ascii")

    # Place with C=1 — cursor should NOT move.
    responses = write_chunked(f"a=T,f=32,s={width},v={height},i=50,C=1", pixels)
    print("  ✓ Placed 16x16 image with C=1 (cursor should not move)")
    print("  This text should appear at the same line as the image.\n")


def run_all():
    """Run all tests in sequence."""
    print()
    print("╔══════════════════════════════════════════════════════════╗")
    print("║       Kitty Graphics Protocol — Manual Test Suite       ║")
    print("║                                                         ║")
    print("║  Run inside the alacritty-kitty fork to test graphics.  ║")
    print("║  Build: cargo build                                     ║")
    print("║  Launch: ./target/debug/alacritty                       ║")
    print("╚══════════════════════════════════════════════════════════╝")
    print()

    test_query()
    test_rgba()
    test_rgb()
    test_png()
    test_chunked()
    test_zlib()
    test_transmit_then_display()
    test_delete()
    test_quiet_modes()
    test_cursor_no_move()

    print("═" * 60)
    print("ALL TESTS SENT.")
    print()
    print("If you see colored squares above, the protocol is working!")
    print("If you only see text, check the debug log output:")
    print("  RUST_LOG=debug ./target/debug/alacritty 2>&1 | grep kitty")
    print("═" * 60)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        run_all()
    else:
        cmd = sys.argv[1]
        dispatch = {
            "query": test_query,
            "rgba": test_rgba,
            "rgb": test_rgb,
            "png": test_png,
            "chunked": test_chunked,
            "zlib": test_zlib,
            "transmit": test_transmit_then_display,
            "delete": test_delete,
            "quiet": test_quiet_modes,
            "cursor": test_cursor_no_move,
            "all": run_all,
        }

        if cmd == "file":
            if len(sys.argv) < 3:
                print("Usage: test_kitty_graphics.py file <path_to_image.png>")
                sys.exit(1)
            test_file(sys.argv[2])
        elif cmd in dispatch:
            dispatch[cmd]()
        else:
            print(f"Unknown test: {cmd}")
            print(f"Available: {', '.join(sorted(dispatch.keys()))}, file <path>")
            sys.exit(1)