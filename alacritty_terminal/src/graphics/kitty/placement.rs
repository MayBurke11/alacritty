//! Kitty graphics image placement and ID resolution.

use crate::event::EventListener;
use crate::graphics::kitty::state::KittyState;
use crate::graphics::kitty_parser::KittyCommand;
use crate::graphics::{ColorType, GraphicData};
use crate::term::Term;

/// Place an image on the terminal grid.
pub fn place_image<L: EventListener>(
    term: &mut Term<L>,
    image_id: u32,
    cmd: &KittyCommand,
) -> Result<(), String> {
    let stored = term
        .graphics
        .kitty_state
        .get_image(image_id)
        .ok_or_else(|| format!("image id={image_id} not in storage"))?;

    let src_data = &stored.data;

    let (pixels, width, height, color_type) = if cmd.src_x != 0
        || cmd.src_y != 0
        || (cmd.src_w != 0 && cmd.src_w != src_data.width as u32)
        || (cmd.src_h != 0 && cmd.src_h != src_data.height as u32)
    {
        crop_image(src_data, cmd)?
    } else {
        (
            src_data.pixels.clone(),
            src_data.width,
            src_data.height,
            src_data.color_type,
        )
    };

    let graphic_id = term.graphics.next_id();

    let graphic = GraphicData {
        id: graphic_id,
        width,
        height,
        color_type,
        pixels,
        is_opaque: color_type == ColorType::Rgb,
    };

    let no_cursor_move = cmd.cursor_movement == 1;
    let saved_cursor = if no_cursor_move {
        Some(term.grid().cursor.point)
    } else {
        None
    };

    crate::graphics::insert_graphic(term, graphic, None);

    if let Some(point) = saved_cursor {
        term.grid_mut().cursor.point = point;
    }

    Ok(())
}

/// Crop an image according to source rectangle parameters.
pub fn crop_image(
    src: &GraphicData,
    cmd: &KittyCommand,
) -> Result<(Vec<u8>, usize, usize, ColorType), String> {
    let bpp = match src.color_type {
        ColorType::Rgba => 4,
        ColorType::Rgb => 3,
    };

    let sx = cmd.src_x as usize;
    let sy = cmd.src_y as usize;
    let sw = if cmd.src_w == 0 { src.width - sx } else { cmd.src_w as usize };
    let sh = if cmd.src_h == 0 { src.height - sy } else { cmd.src_h as usize };

    if sx + sw > src.width || sy + sh > src.height {
        return Err(format!(
            "source rect ({sx},{sy},{sw},{sh}) exceeds image ({}x{})",
            src.width, src.height
        ));
    }

    if sw == 0 || sh == 0 {
        return Err("source rect has zero width or height".into());
    }

    let src_stride = src.width * bpp;
    let dst_stride = sw * bpp;
    let mut cropped = Vec::with_capacity(sh * dst_stride);

    for row in sy..sy + sh {
        let src_start = row * src_stride + sx * bpp;
        let src_end = src_start + dst_stride;
        cropped.extend_from_slice(&src.pixels[src_start..src_end]);
    }

    Ok((cropped, sw, sh, src.color_type))
}

/// Resolve or auto-assign an image ID for a transmit command.
pub fn resolve_or_assign_id<L: EventListener>(term: &mut Term<L>, cmd: &KittyCommand) -> u32 {
    let state = &mut term.graphics.kitty_state;

    let image_id = if cmd.image_id != 0 {
        cmd.image_id
    } else {
        state.next_id()
    };

    if cmd.image_number != 0 {
        state.number_to_id.insert(cmd.image_number, image_id);
    }

    image_id
}

/// Resolve an image ID from a command, checking both `i=` and `I=`.
pub fn resolve_image_id(state: &KittyState, cmd: &KittyCommand) -> Option<u32> {
    if cmd.image_id != 0 {
        Some(cmd.image_id)
    } else if cmd.image_number != 0 {
        state.resolve_number(cmd.image_number)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{GraphicId, ColorType};

    #[test]
    fn crop_image_basic() {
        let pixels: Vec<u8> = [255, 0, 0, 255].repeat(16);
        let src = GraphicData {
            id: GraphicId(0), width: 4, height: 4,
            color_type: ColorType::Rgba, pixels, is_opaque: false,
        };
        let cmd = KittyCommand {
            src_x: 1, src_y: 1, src_w: 2, src_h: 2, ..Default::default()
        };
        let (cropped, w, h, ct) = crop_image(&src, &cmd).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 2);
        assert_eq!(ct, ColorType::Rgba);
        assert_eq!(cropped.len(), 2 * 2 * 4);
    }

    #[test]
    fn crop_image_out_of_bounds() {
        let src = GraphicData {
            id: GraphicId(0), width: 4, height: 4,
            color_type: ColorType::Rgba, pixels: vec![0; 64], is_opaque: false,
        };
        let cmd = KittyCommand {
            src_x: 3, src_y: 3, src_w: 3, src_h: 3, ..Default::default()
        };
        assert!(crop_image(&src, &cmd).is_err());
    }
}