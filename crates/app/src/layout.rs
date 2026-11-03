//! Pixel layout of the placeholder microwave sprite (Phase 1, task 5).
//!
//! The first runnable version draws each flyer as a few solid rectangles
//! (body, door glass, control panel, knob) copied onto the surface, so no
//! shader is needed yet. This module turns a [`SpriteInstance`] into pixel
//! rectangles and clips them to the surface. It has no platform dependency
//! and is tested on every host.

use flying_microwave_core::sim::SpriteInstance;

/// Background color (8-bit RGB). Dark, as guideline G-5 asks.
pub const BACKGROUND_RGB: [u8; 3] = [6, 8, 18];

/// One rectangle of the placeholder sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The cabinet.
    Body,
    /// The door window.
    Glass,
    /// The control panel to the right of the door.
    Panel,
    /// A knob on the control panel.
    Knob,
}

/// Parts in drawing order (later parts are drawn on top).
pub const PARTS: [Part; 4] = [Part::Body, Part::Glass, Part::Panel, Part::Knob];

impl Part {
    /// Color of the part (8-bit RGB).
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Part::Body => [170, 170, 175],
            Part::Glass => [35, 40, 55],
            Part::Panel => [120, 120, 128],
            Part::Knob => [230, 180, 60],
        }
    }

    /// Left, top, width and height as fractions of the sprite side.
    fn frame(self) -> (f32, f32, f32, f32) {
        match self {
            Part::Body => (0.0, 0.15, 1.0, 0.70),
            Part::Glass => (0.08, 0.25, 0.56, 0.50),
            Part::Panel => (0.70, 0.25, 0.22, 0.50),
            Part::Knob => (0.75, 0.32, 0.12, 0.12),
        }
    }
}

/// A rectangle in surface pixels. It may lie partly or wholly outside the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// The visible part of a [`Rect`]: copy `w` x `h` pixels from (`src_x`,
/// `src_y`) of the part's pixels to (`dst_x`, `dst_y`) of the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Copy {
    pub src_x: u32,
    pub src_y: u32,
    pub dst_x: u32,
    pub dst_y: u32,
    pub w: u32,
    pub h: u32,
}

/// Sprite side in pixels for a sprite of `size` screen heights (at least 1).
pub fn side_px(size: f32, height_px: u32) -> u32 {
    // `as` saturates: a negative or NaN product becomes 0, then 1.
    ((size * height_px as f32).round() as u32).max(1)
}

/// Width and height of `part` in pixels for a sprite side of `side` pixels (each at least 1).
pub fn part_size(part: Part, side: u32) -> (u32, u32) {
    let (_, _, w, h) = part.frame();
    let scaled = |fraction: f32| ((fraction * side as f32).round() as u32).max(1);
    (scaled(w), scaled(h))
}

/// Center of the sprite in surface pixels.
pub fn center_px(instance: &SpriteInstance, height_px: u32) -> (f32, f32) {
    let height = height_px as f32;
    (instance.x * height, instance.y * height)
}

/// Where `part` of a sprite of side `side` centered at `center` lies.
pub fn part_rect(part: Part, center: (f32, f32), side: u32) -> Rect {
    let (fx, fy, _, _) = part.frame();
    let half = side as f32 / 2.0;
    // `as` saturates, so far-away sprites stay far away instead of wrapping.
    let left = (center.0 - half).round() as i32;
    let top = (center.1 - half).round() as i32;
    let (w, h) = part_size(part, side);
    Rect {
        x: left.saturating_add((fx * side as f32).round() as i32),
        y: top.saturating_add((fy * side as f32).round() as i32),
        w,
        h,
    }
}

/// Clips `rect` to a `width` x `height` surface. None when nothing is visible.
pub fn clip(rect: Rect, width: u32, height: u32) -> Option<Copy> {
    // i64 holds every i32 + u32 sum, so nothing overflows.
    let span = |start: i32, len: u32, limit: u32| {
        let from = i64::from(start).max(0);
        let to = (i64::from(start) + i64::from(len)).min(i64::from(limit));
        (to > from).then(|| {
            // Both differences are within 0..=u32::MAX.
            (
                (from - i64::from(start)) as u32,
                from as u32,
                (to - from) as u32,
            )
        })
    };
    let (src_x, dst_x, w) = span(rect.x, rect.w, width)?;
    let (src_y, dst_y, h) = span(rect.y, rect.h, height)?;
    Some(Copy {
        src_x,
        src_y,
        dst_x,
        dst_y,
        w,
        h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_is_the_size_in_pixels_and_at_least_one() {
        assert_eq!(side_px(0.12, 1200), 144);
        assert_eq!(side_px(0.12, 1024), 123);
        assert_eq!(side_px(0.0001, 10), 1);
    }

    #[test]
    fn part_rect_has_the_part_size() {
        for part in PARTS {
            let rect = part_rect(part, (500.0, 400.0), 144);
            assert_eq!((rect.w, rect.h), part_size(part, 144), "{part:?}");
        }
    }

    #[test]
    fn body_is_centered_and_contains_the_other_parts() {
        let body = part_rect(Part::Body, (500.0, 400.0), 144);
        assert_eq!((body.x, body.w), (428, 144));
        let mid_y = body.y as f32 + body.h as f32 / 2.0;
        assert!((mid_y - 400.0).abs() <= 1.0, "body middle at {mid_y}");
        for part in [Part::Glass, Part::Panel, Part::Knob] {
            let r = part_rect(part, (500.0, 400.0), 144);
            assert!(r.x >= body.x && r.y >= body.y, "{part:?} {r:?}");
            assert!(r.x + r.w as i32 <= body.x + body.w as i32, "{part:?} {r:?}");
            assert!(r.y + r.h as i32 <= body.y + body.h as i32, "{part:?} {r:?}");
        }
    }

    #[test]
    fn center_converts_screen_heights_to_pixels() {
        let instance = SpriteInstance {
            x: 0.5,
            y: 0.25,
            size: 0.12,
            flap: 0.0,
            color: 0,
        };
        assert_eq!(center_px(&instance, 1200), (600.0, 300.0));
    }

    #[test]
    fn clip_keeps_a_rectangle_inside_the_surface() {
        let rect = Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        };
        assert_eq!(
            clip(rect, 100, 100),
            Some(Copy {
                src_x: 0,
                src_y: 0,
                dst_x: 10,
                dst_y: 20,
                w: 30,
                h: 40
            })
        );
    }

    #[test]
    fn clip_cuts_the_left_and_top_edges() {
        let rect = Rect {
            x: -5,
            y: -7,
            w: 30,
            h: 40,
        };
        assert_eq!(
            clip(rect, 100, 100),
            Some(Copy {
                src_x: 5,
                src_y: 7,
                dst_x: 0,
                dst_y: 0,
                w: 25,
                h: 33
            })
        );
    }

    #[test]
    fn clip_cuts_the_right_and_bottom_edges() {
        let rect = Rect {
            x: 90,
            y: 80,
            w: 30,
            h: 40,
        };
        assert_eq!(
            clip(rect, 100, 100),
            Some(Copy {
                src_x: 0,
                src_y: 0,
                dst_x: 90,
                dst_y: 80,
                w: 10,
                h: 20
            })
        );
    }

    #[test]
    fn clip_drops_a_rectangle_outside_the_surface() {
        for (x, y) in [(-30, 0), (100, 0), (0, -40), (0, 100), (-1000, -1000)] {
            let rect = Rect { x, y, w: 30, h: 40 };
            assert_eq!(clip(rect, 100, 100), None, "({x}, {y})");
        }
    }

    #[test]
    fn clip_handles_extreme_positions() {
        let rect = Rect {
            x: i32::MIN,
            y: i32::MAX,
            w: u32::MAX,
            h: u32::MAX,
        };
        assert_eq!(clip(rect, 100, 100), None);
        let rect = Rect {
            x: i32::MIN,
            y: i32::MIN,
            w: u32::MAX,
            h: u32::MAX,
        };
        assert_eq!(
            clip(rect, 100, 100),
            Some(Copy {
                src_x: 2_147_483_648,
                src_y: 2_147_483_648,
                dst_x: 0,
                dst_y: 0,
                w: 100,
                h: 100
            })
        );
    }
}
