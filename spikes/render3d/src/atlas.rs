//! Packs the face images into one texture atlas with mipmaps.
//!
//! Each image is rasterized by resvg into its own cell. The cells are packed
//! in rows (shelves) with a gutter around each, filled by repeating the edge
//! pixels, so that bilinear filtering and the smaller mip levels do not mix
//! neighboring cells. Mip levels are made on the CPU with a 2x2 box filter
//! of premultiplied RGBA.

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

/// Atlas width in pixels.
const ATLAS_WIDTH: u32 = 4096;
/// Pixels of repeated edge around each cell.
const GUTTER: u32 = 16;
/// Mip levels, including level 0. The gutter of 16 pixels keeps cells apart
/// down to level 4 (1 pixel); the level count stops there.
pub const MIP_LEVELS: u32 = 5;

/// A cell in the atlas, in pixels of level 0.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A rectangle in texture coordinates: u0, v0 (top left), u1, v1.
#[derive(Debug, Clone, Copy)]
pub struct UvRect {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
}

impl UvRect {
    /// The part of this rectangle between the fractions `fx0..fx1` across
    /// and `fy0..fy1` down.
    pub fn sub(&self, fx0: f32, fy0: f32, fx1: f32, fy1: f32) -> UvRect {
        let du = self.u1 - self.u0;
        let dv = self.v1 - self.v0;
        UvRect {
            u0: self.u0 + du * fx0,
            v0: self.v0 + dv * fy0,
            u1: self.u0 + du * fx1,
            v1: self.v0 + dv * fy1,
        }
    }
}

/// An image to put into the atlas: its SVG and its size in pixels.
pub struct Job {
    pub svg: String,
    pub w: u32,
    pub h: u32,
}

pub struct Atlas {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA of each mip level, level 0 first.
    pub levels: Vec<Vec<u8>>,
    pub uvs: Vec<UvRect>,
}

fn round_up(v: u32, to: u32) -> u32 {
    v.div_ceil(to) * to
}

/// Places the cells in rows from the top left. Returns the cells and the
/// atlas height.
fn pack(sizes: &[(u32, u32)]) -> (Vec<Cell>, u32) {
    let mut cells = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut row_h) = (0u32, 0u32, 0u32);
    for &(w, h) in sizes {
        let (outer_w, outer_h) = (w + 2 * GUTTER, h + 2 * GUTTER);
        assert!(outer_w <= ATLAS_WIDTH, "cell of {w} pixels is too wide");
        if x + outer_w > ATLAS_WIDTH {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        cells.push(Cell {
            x: x + GUTTER,
            y: y + GUTTER,
            w,
            h,
        });
        x += outer_w;
        row_h = row_h.max(outer_h);
    }
    // Why a multiple of 2^(levels - 1): every mip level halves exactly.
    let height = round_up(y + row_h, 1 << (MIP_LEVELS - 1));
    (cells, height)
}

fn rasterize(svg: &str, w: u32, h: u32) -> Result<Pixmap, String> {
    let options = usvg::Options {
        // The art references no images; refuse any.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| format!("bad SVG: {e}"))?;
    let size = tree.size();
    let mut pixmap = Pixmap::new(w, h).ok_or("empty cell")?;
    let transform = Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Copies `pixmap` into `atlas` at `cell` and fills the gutter around it by
/// repeating the nearest edge pixel.
fn blit(atlas: &mut [u8], atlas_w: u32, cell: Cell, pixmap: &Pixmap) {
    let src = pixmap.data();
    let g = GUTTER as i64;
    for dy in -g..(cell.h as i64 + g) {
        let sy = dy.clamp(0, cell.h as i64 - 1) as u32;
        let ty = (cell.y as i64 + dy) as u32;
        for dx in -g..(cell.w as i64 + g) {
            let sx = dx.clamp(0, cell.w as i64 - 1) as u32;
            let tx = (cell.x as i64 + dx) as u32;
            let s = ((sy * cell.w + sx) * 4) as usize;
            let t = ((ty * atlas_w + tx) * 4) as usize;
            atlas[t..t + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
}

/// Halves a level with a 2x2 box filter.
fn downsample(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let (nw, nh) = (w / 2, h / 2);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let at = |xx: u32, yy: u32| u32::from(src[((yy * w + xx) * 4 + c) as usize]);
                let sum = at(2 * x, 2 * y)
                    + at(2 * x + 1, 2 * y)
                    + at(2 * x, 2 * y + 1)
                    + at(2 * x + 1, 2 * y + 1);
                out[((y * nw + x) * 4 + c) as usize] = ((sum + 2) / 4) as u8;
            }
        }
    }
    out
}

/// Rasterizes every job and packs them. `uvs[i]` is the cell of `jobs[i]`.
pub fn build(jobs: &[Job]) -> Result<Atlas, String> {
    let sizes: Vec<(u32, u32)> = jobs.iter().map(|j| (j.w, j.h)).collect();
    let (cells, height) = pack(&sizes);
    let width = ATLAS_WIDTH;
    let mut level0 = vec![0u8; (width * height * 4) as usize];
    for (job, cell) in jobs.iter().zip(&cells) {
        let pixmap = rasterize(&job.svg, job.w, job.h)?;
        blit(&mut level0, width, *cell, &pixmap);
    }
    let mut levels = vec![level0];
    let (mut w, mut h) = (width, height);
    for _ in 1..MIP_LEVELS {
        let next = downsample(levels.last().expect("level 0 exists"), w, h);
        w /= 2;
        h /= 2;
        levels.push(next);
    }
    let uvs = cells
        .iter()
        .map(|c| UvRect {
            u0: c.x as f32 / width as f32,
            v0: c.y as f32 / height as f32,
            u1: (c.x + c.w) as f32 / width as f32,
            v1: (c.y + c.h) as f32 / height as f32,
        })
        .collect();
    Ok(Atlas {
        width,
        height,
        levels,
        uvs,
    })
}
