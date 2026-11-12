//! Packs the face images of [`crate::model`] into one texture atlas with
//! mipmaps (ADR-0026). The atlas is made once and shared by every window
//! (plan 0005, D-13).
//!
//! Each image is rasterized by resvg into its own cell. The cells are packed
//! in rows (shelves) with a gutter around each, filled by repeating the edge
//! pixels, so that bilinear filtering and the smaller mip levels do not mix
//! neighboring cells. Mip levels are made on the CPU with a 2x2 box filter
//! of premultiplied RGBA.
//!
//! Everything but the rasterization has no platform dependency and is tested
//! on every host.

use crate::model::UvRect;

/// Atlas width in pixels.
pub const ATLAS_WIDTH: u32 = 4096;
/// Least pixels of repeated edge around each cell.
const GUTTER: u32 = 16;
/// Mip levels, including level 0. The level count stops where one texel is
/// as wide as the gutter.
pub const MIP_LEVELS: u32 = 5;
/// Every outer box (a cell and its gutter) starts and ends on a multiple of
/// this, the pixels of one texel of the smallest level. So no texel of any
/// level covers two boxes, and the gutter keeps the bilinear and trilinear
/// filter at the edge of a cell on the cell's own colors at every level.
/// Anisotropic filtering can reach further along a steep face; how far is
/// up to the GPU, so the edges of far, steep doors are checked by eye with
/// `v0.3.0-alpha.2` (plan 0005, task 8).
const ALIGN: u32 = 1 << (MIP_LEVELS - 1);

/// A premultiplied RGBA image, 8 bits per channel, rows of `width` pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A cell in the atlas, in pixels of level 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cell {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Cell {
    /// Width and height of the box of the cell and its gutter: at least
    /// [`GUTTER`] on each side, rounded up to [`ALIGN`].
    fn outer(w: u32, h: u32) -> (u32, u32) {
        let up = |v: u32| (v + 2 * GUTTER).div_ceil(ALIGN) * ALIGN;
        (up(w), up(h))
    }
}

/// The atlas: every mip level and where each image is.
#[derive(Debug, Clone, PartialEq)]
pub struct Atlas {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA of each mip level, level 0 first.
    pub levels: Vec<Vec<u8>>,
    /// The cell of each image, in the order of the images.
    pub uvs: Vec<UvRect>,
}

/// Places the cells in rows from the top left. Returns the cells and the
/// atlas height.
fn pack(sizes: &[(u32, u32)]) -> Result<(Vec<Cell>, u32), String> {
    // Why the tallest first: images of similar height share a row, so the
    // rows waste less height (the faces differ from 16 to 512 pixels high).
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(Cell::outer(sizes[i].0, sizes[i].1).1));
    let mut cells = vec![
        Cell {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        sizes.len()
    ];
    let (mut x, mut y, mut row_h) = (0u32, 0u32, 0u32);
    for i in order {
        let (w, h) = sizes[i];
        let (outer_w, outer_h) = Cell::outer(w, h);
        if w == 0 || h == 0 || outer_w > ATLAS_WIDTH {
            return Err(format!("an image of {w}x{h} pixels does not fit the atlas"));
        }
        if x + outer_w > ATLAS_WIDTH {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        cells[i] = Cell {
            x: x + GUTTER,
            y: y + GUTTER,
            w,
            h,
        };
        x += outer_w;
        row_h = row_h.max(outer_h);
    }
    // A multiple of ALIGN, as every box is: every mip level halves exactly.
    let height = y + row_h;
    Ok((cells, height))
}

/// Copies `image` into `atlas` at `cell` and fills the rest of its box (the
/// gutter) by repeating the nearest edge pixel.
fn blit(atlas: &mut [u8], atlas_w: u32, cell: Cell, image: &Image) {
    let g = i64::from(GUTTER);
    let (outer_w, outer_h) = Cell::outer(cell.w, cell.h);
    for dy in -g..(i64::from(outer_h) - g) {
        let sy = dy.clamp(0, i64::from(cell.h) - 1) as u32;
        let ty = (i64::from(cell.y) + dy) as u32;
        for dx in -g..(i64::from(outer_w) - g) {
            let sx = dx.clamp(0, i64::from(cell.w) - 1) as u32;
            let tx = (i64::from(cell.x) + dx) as u32;
            let s = ((sy * cell.w + sx) * 4) as usize;
            let t = ((ty * atlas_w + tx) * 4) as usize;
            atlas[t..t + 4].copy_from_slice(&image.rgba[s..s + 4]);
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

/// Packs `images` and makes the mip levels. `uvs[i]` is the cell of `images[i]`.
pub fn build(images: &[Image]) -> Result<Atlas, String> {
    let sizes: Vec<(u32, u32)> = images.iter().map(|i| (i.width, i.height)).collect();
    let (cells, height) = pack(&sizes)?;
    let width = ATLAS_WIDTH;
    let mut level0 = vec![0u8; (width * height * 4) as usize];
    for (image, cell) in images.iter().zip(&cells) {
        if image.rgba.len() != (image.width * image.height * 4) as usize {
            return Err(format!(
                "an image of {}x{} pixels has {} bytes",
                image.width,
                image.height,
                image.rgba.len()
            ));
        }
        blit(&mut level0, width, *cell, image);
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

/// Rasterizes every face image of [`crate::model::Face::all`], in that order.
#[cfg(windows)]
pub fn rasterize_faces() -> Result<Vec<Image>, String> {
    use resvg::tiny_skia::{Pixmap, Transform};
    use resvg::usvg;

    use crate::model::{Face, Sheet, sheets};

    let options = usvg::Options {
        // Why: the assets reference no images; refuse any, so that an asset
        // can never make the screensaver read a file.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let trees = sheets()
        .into_iter()
        .map(|sheet| {
            usvg::Tree::from_str(sheet.svg(), &options)
                .map(|tree| (sheet, tree))
                .map_err(|e| format!("{sheet:?} could not be parsed: {e}"))
        })
        .collect::<Result<Vec<(Sheet, usvg::Tree)>, String>>()?;
    Face::all()
        .into_iter()
        .map(|face| {
            let image = face.image();
            let (_, tree) = trees
                .iter()
                .find(|(sheet, _)| *sheet == image.sheet)
                .ok_or_else(|| format!("no sheet for {face:?}"))?;
            let mut pixmap = Pixmap::new(image.width, image.height)
                .ok_or_else(|| format!("{face:?}: empty image"))?;
            // Scale the face rectangle of the sheet to the pixmap; the rest
            // of the sheet falls outside and is clipped.
            let [x, y, w, h] = image.rect;
            let (sx, sy) = (image.width as f32 / w, image.height as f32 / h);
            let transform = Transform::from_row(sx, 0.0, 0.0, sy, -x * sx, -y * sy);
            resvg::render(tree, transform, &mut pixmap.as_mut());
            Ok(Image {
                width: image.width,
                height: image.height,
                rgba: pixmap.take(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Face;

    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Image {
        Image {
            width,
            height,
            rgba: rgba.repeat((width * height) as usize),
        }
    }

    fn pixel(atlas: &Atlas, level: usize, x: u32, y: u32) -> [u8; 4] {
        let w = atlas.width >> level;
        let i = ((y * w + x) * 4) as usize;
        atlas.levels[level][i..i + 4].try_into().expect("4 bytes")
    }

    #[test]
    fn product_faces_fit_one_atlas_of_bounded_size() {
        let sizes: Vec<(u32, u32)> = Face::all()
            .iter()
            .map(|f| (f.image().width, f.image().height))
            .collect();
        let (cells, height) = pack(&sizes).expect("fits");
        assert_eq!(cells.len(), sizes.len());
        // Every device of the DX12 feature level 11 takes 16384 pixels;
        // the bound keeps the memory near the spike's 35 MB (ADR-0026).
        assert!(height <= 2048, "{height}");
    }

    #[test]
    fn cells_and_gutters_do_not_overlap() {
        let sizes = [(800, 500), (3000, 40), (100, 100), (4064, 10), (5, 7)];
        let (cells, height) = pack(&sizes).expect("fits");
        assert_eq!(height % (1 << (MIP_LEVELS - 1)), 0);
        let outer = |c: &Cell| {
            let (w, h) = Cell::outer(c.w, c.h);
            (c.x - GUTTER, c.y - GUTTER, w, h)
        };
        for (i, a) in cells.iter().enumerate() {
            let (ax, ay, aw, ah) = outer(a);
            assert!(ax + aw <= ATLAS_WIDTH && ay + ah <= height, "{a:?}");
            for v in [ax, ay, aw, ah] {
                assert_eq!(v % ALIGN, 0, "{a:?} is not aligned");
            }
            assert!(aw >= a.w + 2 * GUTTER && ah >= a.h + 2 * GUTTER, "{a:?}");
            assert_eq!((a.w, a.h), sizes[i]);
            for b in &cells[i + 1..] {
                let (bx, by, bw, bh) = outer(b);
                let apart = ax + aw <= bx || bx + bw <= ax || ay + ah <= by || by + bh <= ay;
                assert!(apart, "{a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn mip_texels_around_a_cell_hold_only_that_cell() {
        // Review of plan 0005, task 6: bilinear filtering at the edge of a
        // cell reads the texels next to it at every mip level, so the texels
        // that cover a cell and its gutter must not mix in a neighbor or the
        // empty (transparent) area.
        let faces = Face::all();
        let images: Vec<Image> = faces
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let img = f.image();
                solid(
                    img.width,
                    img.height,
                    [(i * 8) as u8, 255 - i as u8, 7, 255],
                )
            })
            .collect();
        let atlas = build(&images).expect("atlas");
        let (w, h) = (atlas.width as f32, atlas.height as f32);
        for (i, uv) in atlas.uvs.iter().enumerate() {
            let want = [(i * 8) as u8, 255 - i as u8, 7, 255];
            let x0 = (uv.u0 * w).round() as u32;
            let x1 = (uv.u1 * w).round() as u32;
            let y0 = (uv.v0 * h).round() as u32;
            let y1 = (uv.v1 * h).round() as u32;
            for level in 0..MIP_LEVELS as usize {
                // Texels from the one before the cell edge to the one after.
                let texel = |p: u32| p >> level;
                let xs = texel(x0) - 1..=texel(x1 - 1) + 1;
                let ys = texel(y0) - 1..=texel(y1 - 1) + 1;
                for y in ys.clone() {
                    for x in xs.clone() {
                        assert_eq!(
                            pixel(&atlas, level, x, y),
                            want,
                            "{:?} at level {level}, texel ({x}, {y})",
                            faces[i]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn too_wide_or_empty_images_are_rejected() {
        assert!(pack(&[(ATLAS_WIDTH - 2 * GUTTER + 1, 4)]).is_err());
        assert!(pack(&[(0, 4)]).is_err());
        let bad = Image {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        };
        assert!(build(&[bad]).is_err());
    }

    #[test]
    fn gutter_repeats_the_edge_pixels() {
        let mut image = solid(4, 3, [10, 20, 30, 255]);
        // Top-left pixel differs.
        image.rgba[..4].copy_from_slice(&[200, 0, 0, 255]);
        let atlas = build(&[image]).expect("atlas");
        let (x, y) = (GUTTER, GUTTER);
        assert_eq!(pixel(&atlas, 0, x, y), [200, 0, 0, 255]);
        assert_eq!(pixel(&atlas, 0, 0, 0), [200, 0, 0, 255]);
        assert_eq!(pixel(&atlas, 0, x - 1, y + 2), [10, 20, 30, 255]);
        assert_eq!(
            pixel(&atlas, 0, x + 3 + GUTTER, y + 2 + GUTTER),
            [10, 20, 30, 255]
        );
        // The gutter fills the box up to the next multiple of ALIGN (48 for
        // a 4-pixel cell), and the atlas stays empty beyond it.
        assert_eq!(pixel(&atlas, 0, 47, y), [10, 20, 30, 255]);
        assert_eq!(pixel(&atlas, 0, 48, y), [0, 0, 0, 0]);
    }

    #[test]
    fn mip_levels_halve_and_average() {
        let atlas = build(&[solid(64, 64, [100, 50, 0, 200])]).expect("atlas");
        assert_eq!(atlas.levels.len(), MIP_LEVELS as usize);
        for (level, pixels) in atlas.levels.iter().enumerate() {
            let (w, h) = (atlas.width >> level, atlas.height >> level);
            assert_eq!(pixels.len(), (w * h * 4) as usize, "level {level}");
        }
        // The cell center keeps its color at every level.
        for level in 0..MIP_LEVELS as usize {
            let c = (GUTTER + 32) >> level;
            assert_eq!(
                pixel(&atlas, level, c, c),
                [100, 50, 0, 200],
                "level {level}"
            );
        }
        assert_eq!(
            downsample(
                &[0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0, 1, 2, 3, 4],
                2,
                2
            ),
            vec![64, 64, 65, 65]
        );
    }

    #[test]
    fn uvs_point_at_the_cells() {
        let atlas = build(&[solid(100, 50, [1; 4]), solid(30, 60, [2; 4])]).expect("atlas");
        let (w, h) = (atlas.width as f32, atlas.height as f32);
        let uv = atlas.uvs[1];
        let x = ((uv.u0 * w).round() as u32, (uv.u1 * w).round() as u32);
        let y = ((uv.v0 * h).round() as u32, (uv.v1 * h).round() as u32);
        assert_eq!(x.1 - x.0, 30);
        assert_eq!(y.1 - y.0, 60);
        assert_eq!(pixel(&atlas, 0, x.0, y.0), [2; 4]);
        assert_eq!(pixel(&atlas, 0, x.1 - 1, y.1 - 1), [2; 4]);
    }

    #[cfg(windows)]
    #[test]
    fn rasterized_faces_have_the_expected_coverage() {
        let images = rasterize_faces().expect("rasterize");
        assert_eq!(images.len(), crate::model::FACE_COUNT);
        let alpha = |face: Face, fx: f32, fy: f32| {
            let img = &images[face.index()];
            let x = ((img.width as f32 * fx) as u32).min(img.width - 1);
            let y = ((img.height as f32 * fy) as u32).min(img.height - 1);
            img.rgba[((y * img.width + x) * 4 + 3) as usize]
        };
        // The body, cavity, turntable and LED are drawn as opaque without
        // looking at the texel alpha (fs_solid in mesh.wgsl), and the door
        // edges are kept only where opaque (fs_cutout): every pixel of their
        // images must be opaque, or the edges would come out darker or holed.
        let solid = (0..4)
            .flat_map(|c| {
                [
                    Face::Front,
                    Face::Back,
                    Face::Side,
                    Face::Left,
                    Face::Top,
                    Face::Bottom,
                ]
                .map(|f| f(c))
            })
            .chain([Face::Cavity, Face::Turntable, Face::Led, Face::DoorEdge]);
        for face in solid {
            let img = &images[face.index()];
            let clear = img
                .rgba
                .iter()
                .skip(3)
                .step_by(4)
                .filter(|&&a| a != 255)
                .count();
            assert_eq!(clear, 0, "{face:?} has pixels that are not opaque");
        }
        // The door frame is opaque and its window lets the cavity show through.
        assert_eq!(alpha(Face::DoorOuter, 0.02, 0.5), 255);
        let window = alpha(Face::DoorOuter, 0.5, 0.75);
        assert!((10..250).contains(&window), "window alpha {window}");
        // The body colors differ.
        let rgb = |color: u8| {
            let img = &images[Face::Left(color).index()];
            img.rgba[..3].to_vec()
        };
        for a in 0..4 {
            for b in a + 1..4 {
                assert_ne!(rgb(a), rgb(b));
            }
        }
    }
}
