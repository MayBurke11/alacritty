//! Kitty graphics payload decode pipeline.
//!
//! Pipeline: base64 → zlib (optional) → format decode → GraphicData

use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::read::ZlibDecoder;

use crate::graphics::kitty_parser::{Compression, Format, KittyCommand, Medium};
use crate::graphics::{ColorType, GraphicData, MAX_GRAPHIC_DIMENSIONS};

/// Maximum dimensions for a single image.
const MAX_IMAGE_WIDTH: u32 = 10000;
const MAX_IMAGE_HEIGHT: u32 = 10000;

/// Errors that can occur during kitty image decoding.
#[derive(Debug)]
pub enum DecodeError {
    Base64(String),
    Zlib(String),
    Png(String),
    InvalidDimensions(String),
    UnsupportedMedium(Medium),
    MissingDimensions,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::Base64(e) => write!(f, "base64 decode error: {e}"),
            DecodeError::Zlib(e) => write!(f, "zlib decompress error: {e}"),
            DecodeError::Png(e) => write!(f, "PNG decode error: {e}"),
            DecodeError::InvalidDimensions(e) => write!(f, "invalid dimensions: {e}"),
            DecodeError::UnsupportedMedium(m) => write!(f, "unsupported medium: {m:?}"),
            DecodeError::MissingDimensions => write!(f, "missing width/height for raw format"),
        }
    }
}

/// Decode a kitty graphics payload into a `GraphicData`.
///
/// Pipeline: base64 → zlib (optional) → format decode → GraphicData
pub fn decode_payload(cmd: &KittyCommand, raw_payload: &[u8]) -> Result<GraphicData, DecodeError> {
    if cmd.medium != Medium::Direct {
        return Err(DecodeError::UnsupportedMedium(cmd.medium));
    }

    let decoded = BASE64
        .decode(raw_payload)
        .map_err(|e| DecodeError::Base64(e.to_string()))?;

    let decompressed = match cmd.compression {
        Compression::Zlib => {
            let mut decoder = ZlibDecoder::new(&decoded[..]);
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf).map_err(|e| DecodeError::Zlib(e.to_string()))?;
            buf
        },
        Compression::None => decoded,
    };

    match cmd.format {
        Format::Png => decode_png(&decompressed),
        Format::Rgba => decode_raw(&decompressed, cmd.width, cmd.height, 4),
        Format::Rgb => decode_rgb_to_rgba(&decompressed, cmd.width, cmd.height),
    }
}

fn decode_png(data: &[u8]) -> Result<GraphicData, DecodeError> {
    let decoder = png::Decoder::new(data);
    let mut reader = decoder.read_info().map_err(|e| DecodeError::Png(e.to_string()))?;

    let info = reader.info();
    let width = info.width as usize;
    let height = info.height as usize;

    if width > MAX_IMAGE_WIDTH as usize || height > MAX_IMAGE_HEIGHT as usize {
        return Err(DecodeError::InvalidDimensions(format!(
            "PNG {width}x{height} exceeds max {MAX_IMAGE_WIDTH}x{MAX_IMAGE_HEIGHT}"
        )));
    }

    if width > MAX_GRAPHIC_DIMENSIONS[0] || height > MAX_GRAPHIC_DIMENSIONS[1] {
        return Err(DecodeError::InvalidDimensions(format!(
            "PNG {width}x{height} exceeds max graphic dimensions"
        )));
    }

    let mut buf = vec![0u8; reader.output_buffer_size()];
    let output_info = reader.next_frame(&mut buf).map_err(|e| DecodeError::Png(e.to_string()))?;
    buf.truncate(output_info.buffer_size());

    let (pixels, color_type) = match output_info.color_type {
        png::ColorType::Rgba => (buf, ColorType::Rgba),
        png::ColorType::Rgb => {
            let rgba = rgb_bytes_to_rgba(&buf);
            (rgba, ColorType::Rgba)
        },
        png::ColorType::GrayscaleAlpha => {
            let rgba: Vec<u8> = buf.chunks_exact(2).flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]]).collect();
            (rgba, ColorType::Rgba)
        },
        png::ColorType::Grayscale => {
            let rgba: Vec<u8> = buf.iter().flat_map(|&g| [g, g, g, 255]).collect();
            (rgba, ColorType::Rgba)
        },
        png::ColorType::Indexed => {
            return Err(DecodeError::Png("indexed PNG not directly supported".into()));
        },
    };

    Ok(GraphicData {
        id: crate::graphics::GraphicId(0),
        width,
        height,
        color_type,
        pixels,
        is_opaque: color_type == ColorType::Rgb,
    })
}

fn decode_raw(
    data: &[u8],
    width: u32,
    height: u32,
    bpp: usize,
) -> Result<GraphicData, DecodeError> {
    let w = width as usize;
    let h = height as usize;

    if w == 0 || h == 0 {
        return Err(DecodeError::MissingDimensions);
    }

    let expected = w * h * bpp;
    if data.len() != expected {
        return Err(DecodeError::InvalidDimensions(format!(
            "expected {expected} bytes for {w}x{h}x{bpp}, got {}",
            data.len()
        )));
    }

    if w > MAX_IMAGE_WIDTH as usize || h > MAX_IMAGE_HEIGHT as usize {
        return Err(DecodeError::InvalidDimensions(format!(
            "{w}x{h} exceeds max {MAX_IMAGE_WIDTH}x{MAX_IMAGE_HEIGHT}"
        )));
    }

    let color_type = if bpp == 4 { ColorType::Rgba } else { ColorType::Rgb };

    Ok(GraphicData {
        id: crate::graphics::GraphicId(0),
        width: w,
        height: h,
        color_type,
        pixels: data.to_vec(),
        is_opaque: bpp == 3,
    })
}

fn decode_rgb_to_rgba(
    data: &[u8],
    width: u32,
    height: u32,
) -> Result<GraphicData, DecodeError> {
    let w = width as usize;
    let h = height as usize;

    if w == 0 || h == 0 {
        return Err(DecodeError::MissingDimensions);
    }

    let expected = w * h * 3;
    if data.len() != expected {
        return Err(DecodeError::InvalidDimensions(format!(
            "expected {expected} bytes for {w}x{h}x3 (RGB), got {}",
            data.len()
        )));
    }

    if w > MAX_IMAGE_WIDTH as usize || h > MAX_IMAGE_HEIGHT as usize {
        return Err(DecodeError::InvalidDimensions(format!(
            "{w}x{h} exceeds max {MAX_IMAGE_WIDTH}x{MAX_IMAGE_HEIGHT}"
        )));
    }

    let rgba = rgb_bytes_to_rgba(data);

    Ok(GraphicData {
        id: crate::graphics::GraphicId(0),
        width: w,
        height: h,
        color_type: ColorType::Rgba,
        pixels: rgba,
        is_opaque: true,
    })
}

/// Convert an RGB byte slice to RGBA (fully opaque).
pub fn rgb_bytes_to_rgba(rgb: &[u8]) -> Vec<u8> {
    let pixel_count = rgb.len() / 3;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for chunk in rgb.chunks_exact(3) {
        rgba.extend_from_slice(chunk);
        rgba.push(255);
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::kitty_parser::{Action, Compression, Format, KittyCommand};

    #[test]
    fn decode_raw_rgba() {
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 255,
            0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let encoded = BASE64.encode(&pixels);
        let cmd = KittyCommand {
            format: Format::Rgba, width: 2, height: 2,
            payload: encoded.into_bytes(), ..Default::default()
        };
        let data = decode_payload(&cmd, &cmd.payload).unwrap();
        assert_eq!(data.width, 2);
        assert_eq!(data.height, 2);
        assert_eq!(data.pixels, pixels);
    }

    #[test]
    fn decode_raw_rgb() {
        let rgb_pixels: Vec<u8> = vec![255, 0, 0, 0, 255, 0];
        let encoded = BASE64.encode(&rgb_pixels);
        let cmd = KittyCommand {
            format: Format::Rgb, width: 2, height: 1,
            payload: encoded.into_bytes(), ..Default::default()
        };
        let data = decode_payload(&cmd, &cmd.payload).unwrap();
        assert_eq!(data.pixels, vec![255, 0, 0, 255, 0, 255, 0, 255]);
    }

    #[test]
    fn decode_dimension_mismatch() {
        let encoded = BASE64.encode(&[0u8; 10]);
        let cmd = KittyCommand {
            format: Format::Rgba, width: 2, height: 2,
            payload: encoded.into_bytes(), ..Default::default()
        };
        assert!(decode_payload(&cmd, &cmd.payload).is_err());
    }

    #[test]
    fn decode_missing_dimensions() {
        let encoded = BASE64.encode(&[0u8; 16]);
        let cmd = KittyCommand {
            format: Format::Rgba, width: 0, height: 0,
            payload: encoded.into_bytes(), ..Default::default()
        };
        assert!(decode_payload(&cmd, &cmd.payload).is_err());
    }

    #[test]
    fn decode_zlib_compressed() {
        use flate2::write::ZlibEncoder;
        use std::io::Write;
        let pixels: Vec<u8> = vec![255, 128, 64, 255];
        let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&pixels).unwrap();
        let compressed = encoder.finish().unwrap();
        let encoded = BASE64.encode(&compressed);
        let cmd = KittyCommand {
            format: Format::Rgba, compression: Compression::Zlib,
            width: 1, height: 1, payload: encoded.into_bytes(), ..Default::default()
        };
        let data = decode_payload(&cmd, &cmd.payload).unwrap();
        assert_eq!(data.pixels, pixels);
    }

    #[test]
    fn decode_png_image() {
        let mut png_buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_buf, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 0, 0, 255]).unwrap();
        }
        let encoded = BASE64.encode(&png_buf);
        let cmd = KittyCommand {
            format: Format::Png, payload: encoded.into_bytes(), ..Default::default()
        };
        let data = decode_payload(&cmd, &cmd.payload).unwrap();
        assert_eq!(data.pixels, vec![255, 0, 0, 255]);
    }

    #[test]
    fn rgb_to_rgba_conversion() {
        let rgb = vec![255, 0, 0, 0, 255, 0];
        let rgba = rgb_bytes_to_rgba(&rgb);
        assert_eq!(rgba, vec![255, 0, 0, 255, 0, 255, 0, 255]);
    }
}