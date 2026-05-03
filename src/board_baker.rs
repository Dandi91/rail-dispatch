use crate::assets::AssetHandles;
use crate::display::DEFAULT_LAMP_HEIGHT;
use crate::level::LampData;
use bevy::prelude::*;
use std::time::Instant;

const LAMP_HEIGHT_PX: u32 = DEFAULT_LAMP_HEIGHT as u32;

#[derive(Resource)]
pub struct OriginalBoard(pub Image);

pub struct CoverImages {
    pub block_left: Image,
    pub block_mid: Image,
    pub block_right: Image,
    pub signal_left: Image,
    pub signal_mid: Image,
    pub signal_right: Image,
}

impl CoverImages {
    pub fn from_assets(images: &Assets<Image>, handles: &AssetHandles) -> Option<Self> {
        Some(Self {
            block_left: images.get(&handles.cover_block_left)?.clone(),
            block_mid: images.get(&handles.cover_block_mid)?.clone(),
            block_right: images.get(&handles.cover_block_right)?.clone(),
            signal_left: images.get(&handles.cover_signal_left)?.clone(),
            signal_mid: images.get(&handles.cover_signal_mid)?.clone(),
            signal_right: images.get(&handles.cover_signal_right)?.clone(),
        })
    }

    fn for_lamp(&self, lamp_id: u32) -> (&Image, &Image, &Image) {
        if lamp_id >= 100 {
            (&self.signal_left, &self.signal_mid, &self.signal_right)
        } else {
            (&self.block_left, &self.block_mid, &self.block_right)
        }
    }
}

/// Bake all lamp covers into the board image asset, restoring from `original`
/// first so re-bakes don't accumulate. Returns `false` if any required image
/// asset is not yet loaded.
pub fn bake_into_assets(
    images: &mut Assets<Image>,
    handles: &AssetHandles,
    original: &OriginalBoard,
    lamps: &[LampData],
    background: Srgba,
) -> bool {
    let Some(covers) = CoverImages::from_assets(images, handles) else {
        return false;
    };
    let Some(board) = images.get_mut(&handles.board) else {
        return false;
    };
    let now = Instant::now();
    bake_board(board, &original.0, &covers, lamps, background);
    info!("Baked board in {} us", now.elapsed().as_micros());
    true
}

pub fn bake_board(
    board: &mut Image,
    original_board: &Image,
    covers: &CoverImages,
    lamps: &[LampData],
    background: Srgba,
) {
    reset_board(board, original_board);

    let bg_bytes = [
        (background.red.clamp(0.0, 1.0) * 255.0).round() as u8,
        (background.green.clamp(0.0, 1.0) * 255.0).round() as u8,
        (background.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
        255,
    ];

    for lamp in lamps {
        let (left, mid, right) = covers.for_lamp(lamp.id);
        let cover_w = lamp.width.max(1) as u32;
        let composed = compose_cover(left, mid, right, cover_w, bg_bytes);
        paste_cover(
            board,
            &composed,
            cover_w,
            LAMP_HEIGHT_PX,
            lamp.x as f32,
            lamp.y as f32,
            lamp.rotation as f32,
        );
    }
}

fn reset_board(board: &mut Image, original_board: &Image) {
    let Some(src) = original_board.data.as_ref() else {
        return;
    };
    let Some(dst) = board.data.as_mut() else { return };
    if dst.len() == src.len() {
        dst.copy_from_slice(src);
    } else {
        *dst = src.clone();
    }
}

/// Compose a `cover_w × LAMP_HEIGHT_PX` RGBA8 buffer from left/mid/right, with
/// magenta (#FF00FF) replaced by `bg_bytes` so subsequent sampling does not
/// blend magenta into neighboring pixels.
fn compose_cover(left: &Image, mid: &Image, right: &Image, cover_w: u32, bg_bytes: [u8; 4]) -> Vec<u8> {
    let h = LAMP_HEIGHT_PX;
    let mut out = vec![0u8; (cover_w * h * 4) as usize];

    let lw = left.width().min(cover_w);
    let rw = right.width().min(cover_w.saturating_sub(lw));
    let mid_start = lw;
    let mid_end = cover_w - rw;

    let put = |out: &mut [u8], dst_x: u32, dst_y: u32, src: &[u8]| {
        let off = ((dst_y * cover_w + dst_x) * 4) as usize;
        let pixel = classify(src, bg_bytes);
        out[off..off + 4].copy_from_slice(&pixel);
    };

    let read_pixel = |img: &Image, x: u32, y: u32| -> [u8; 4] {
        let data = img.data.as_ref().expect("cover image data missing");
        let off = ((y * img.width() + x) * 4) as usize;
        [data[off], data[off + 1], data[off + 2], data[off + 3]]
    };

    for y in 0..h {
        // Left section.
        for x in 0..lw {
            let src_y = y.min(left.height().saturating_sub(1));
            put(&mut out, x, y, &read_pixel(left, x, src_y));
        }
        // Mid stretched across [mid_start, mid_end).
        if mid_end > mid_start {
            let src_y = y.min(mid.height().saturating_sub(1));
            // Mid is typically 1px wide; if wider, repeat-tile.
            let mw = mid.width().max(1);
            for x in mid_start..mid_end {
                let src_x = (x - mid_start) % mw;
                put(&mut out, x, y, &read_pixel(mid, src_x, src_y));
            }
        }
        // Right section.
        for i in 0..rw {
            let src_y = y.min(right.height().saturating_sub(1));
            let dst_x = cover_w - rw + i;
            put(&mut out, dst_x, y, &read_pixel(right, i, src_y));
        }
    }

    out
}

/// Replace magenta (#FF00FF, opaque) with `bg_bytes`. Leave other pixels intact
/// (including PNG-transparent ones, which keep their alpha=0).
fn classify(src: &[u8], bg_bytes: [u8; 4]) -> [u8; 4] {
    let r = src[0];
    let g = src[1];
    let b = src[2];
    let a = src[3];
    if r == 255 && g == 0 && b == 255 && a == 255 {
        bg_bytes
    } else {
        [r, g, b, a]
    }
}

fn paste_cover(
    board: &mut Image,
    composed: &[u8],
    cover_w: u32,
    cover_h: u32,
    lamp_x: f32,
    lamp_y: f32,
    rotation_deg: f32,
) {
    let board_w = board.width();
    let board_h = board.height();
    let Some(board_data) = board.data.as_mut() else { return };

    if rotation_deg == 0.0 || rotation_deg == 180.0 {
        let flip = rotation_deg == 180.0;
        let dx0 = lamp_x.round() as i32;
        let dy0 = lamp_y.round() as i32;
        for cy in 0..cover_h {
            for cx in 0..cover_w {
                let (sx, sy) = if flip {
                    (cover_w - 1 - cx, cover_h - 1 - cy)
                } else {
                    (cx, cy)
                };
                let bx = dx0 + cx as i32;
                let by = dy0 + cy as i32;
                if bx < 0 || by < 0 || bx >= board_w as i32 || by >= board_h as i32 {
                    continue;
                }
                let src_off = ((sy * cover_w + sx) * 4) as usize;
                let dst_off = ((by as u32 * board_w + bx as u32) * 4) as usize;
                board_data[dst_off..dst_off + 4].copy_from_slice(&composed[src_off..src_off + 4]);
            }
        }
        return;
    }

    // Rotated path: bilinear-sample the composed cover into board pixels inside
    // the rotated bounding box. Rotation is around the cover's center, which
    // sits at (lamp_x + cover_w/2, lamp_y + cover_h/2) in board space.
    let theta = rotation_deg.to_radians();
    let (sin, cos) = theta.sin_cos();
    let cw = cover_w as f32;
    let ch = cover_h as f32;
    let cx_centre = lamp_x + cw * 0.5;
    let cy_centre = lamp_y + ch * 0.5;

    // Bounding box of the rotated cover rectangle.
    let corners = [(0.0_f32, 0.0_f32), (cw, 0.0), (0.0, ch), (cw, ch)];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (lx, ly) in corners {
        let dx = lx - cw * 0.5;
        let dy = ly - ch * 0.5;
        let bx = cx_centre + dx * cos - dy * sin;
        let by = cy_centre + dx * sin + dy * cos;
        min_x = min_x.min(bx);
        min_y = min_y.min(by);
        max_x = max_x.max(bx);
        max_y = max_y.max(by);
    }
    let x0 = (min_x.floor() as i32).max(0);
    let y0 = (min_y.floor() as i32).max(0);
    let x1 = (max_x.ceil() as i32).min(board_w as i32);
    let y1 = (max_y.ceil() as i32).min(board_h as i32);

    for by in y0..y1 {
        for bx in x0..x1 {
            // Inverse rotation around the cover center.
            let px = bx as f32 + 0.5 - cx_centre;
            let py = by as f32 + 0.5 - cy_centre;
            let lx = px * cos + py * sin + cw * 0.5 - 0.5;
            let ly = -px * sin + py * cos + ch * 0.5 - 0.5;
            if lx < -0.5 || ly < -0.5 || lx > cw - 0.5 || ly > ch - 0.5 {
                continue;
            }
            let sample = bilinear(composed, cover_w, cover_h, lx, ly);
            let dst_off = ((by as u32 * board_w + bx as u32) * 4) as usize;
            board_data[dst_off..dst_off + 4].copy_from_slice(&sample);
        }
    }
}

/// Bilinear sample with premultiplied alpha to avoid dark fringes where opaque
/// pixels sit next to PNG-transparent (alpha=0, RGB=0) ones.
fn bilinear(buf: &[u8], w: u32, h: u32, x: f32, y: f32) -> [u8; 4] {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x0c = x0.clamp(0, w as i32 - 1) as u32;
    let y0c = y0.clamp(0, h as i32 - 1) as u32;
    let x1c = (x0 + 1).clamp(0, w as i32 - 1) as u32;
    let y1c = (y0 + 1).clamp(0, h as i32 - 1) as u32;
    let fx = (x - x0 as f32).clamp(0.0, 1.0);
    let fy = (y - y0 as f32).clamp(0.0, 1.0);

    // Read as straight RGBA (0..255), then convert to premultiplied RGBA in 0..1.
    let p = |xx: u32, yy: u32| -> [f32; 4] {
        let off = ((yy * w + xx) * 4) as usize;
        let a = buf[off + 3] as f32 / 255.0;
        [
            buf[off] as f32 / 255.0 * a,
            buf[off + 1] as f32 / 255.0 * a,
            buf[off + 2] as f32 / 255.0 * a,
            a,
        ]
    };
    let p00 = p(x0c, y0c);
    let p10 = p(x1c, y0c);
    let p01 = p(x0c, y1c);
    let p11 = p(x1c, y1c);

    let mut blended = [0.0_f32; 4];
    for c in 0..4 {
        let top = p00[c] * (1.0 - fx) + p10[c] * fx;
        let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
        blended[c] = top * (1.0 - fy) + bot * fy;
    }

    let a = blended[3];
    if a <= 0.0 {
        return [0, 0, 0, 0];
    }
    let inv = 1.0 / a;
    [
        (blended[0] * inv * 255.0).round().clamp(0.0, 255.0) as u8,
        (blended[1] * inv * 255.0).round().clamp(0.0, 255.0) as u8,
        (blended[2] * inv * 255.0).round().clamp(0.0, 255.0) as u8,
        (a * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}
