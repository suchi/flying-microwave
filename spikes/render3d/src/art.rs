//! Face images of the microwave, written as short SVG documents and
//! rasterized with resvg. All art is made here for the spike (no outside
//! material), following the colors and layout of `assets/sprites/*.svg`.
//!
//! Units: the front face is 240 x 160 SVG units, as in the 2D sprite, and
//! 160 SVG units are 1.0 in the 3D model.

/// SVG units per model unit.
pub const SVG_PER_UNIT: f32 = 160.0;
/// Body size in SVG units: width, height and depth (the 2D sprite draws a
/// 240 x 160 front and a 210 units deep body).
pub const BODY_SVG: [f32; 3] = [240.0, 160.0, 210.0];
/// The door on the front image: left, top, width, height (SVG units).
pub const DOOR_RECT: [f32; 4] = [10.0, 10.0, 156.0, 140.0];
/// The opening of the cavity behind the door, on the front image
/// (the door window of the 2D sprite).
pub const OPENING_RECT: [f32; 4] = [24.0, 24.0, 128.0, 112.0];
/// The LED window on the front image.
pub const LED_RECT: [f32; 4] = [178.0, 18.0, 48.0, 22.0];

/// A body color: the top and bottom stops of the front gradient of
/// `assets/sprites/microwave-body-<name>.svg`.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub name: &'static str,
    pub top: &'static str,
    pub bottom: &'static str,
}

pub const PALETTES: [Palette; 4] = [
    Palette {
        name: "silver",
        top: "#b3b9c5",
        bottom: "#7c8392",
    },
    Palette {
        name: "graphite",
        top: "#555c6b",
        bottom: "#2f333d",
    },
    Palette {
        name: "white",
        top: "#eceef2",
        bottom: "#c3c8d1",
    },
    Palette {
        name: "mint",
        top: "#a9e3d7",
        bottom: "#6cbcad",
    },
];

const XMLNS: &str = "http://www.w3.org/2000/svg";

fn svg(w: f32, h: f32, body: &str) -> String {
    format!(r#"<svg xmlns="{XMLNS}" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{body}</svg>"#)
}

fn body_gradient(p: &Palette) -> String {
    format!(
        r#"<linearGradient id="body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient>"#,
        p.top, p.bottom
    )
}

/// The front: body color, the dark recess under the door, the control panel
/// with buttons and a dial. The LED digits are a separate, unlit quad.
pub fn front(p: &Palette) -> String {
    let [w, h, _] = BODY_SVG;
    let [dx, dy, dw, dh] = DOOR_RECT;
    let mut s = format!(
        r##"<defs>{}<linearGradient id="panel" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2a303b"/><stop offset="1" stop-color="#1a1e26"/></linearGradient></defs>
<rect width="{w}" height="{h}" fill="url(#body)"/>
<rect width="{w}" height="2" fill="#ffffff" opacity="0.25"/>
<rect x="{dx}" y="{dy}" width="{dw}" height="{dh}" rx="6" fill="#1a1f28"/>
<rect x="172" y="10" width="60" height="140" rx="5" fill="url(#panel)"/>
<rect x="177" y="17" width="50" height="24" rx="3" fill="#06110f"/>"##,
        body_gradient(p)
    );
    for row in 0..3 {
        for col in 0..3 {
            let x = 180.0 + 15.5 * col as f32;
            let y = 50.0 + 15.0 * row as f32;
            s += &format!(
                r##"<rect x="{x}" y="{y}" width="12" height="10" rx="2" fill="#4a5262"/>"##
            );
        }
    }
    s += r##"<circle cx="202" cy="120" r="14" fill="#9aa1ae" stroke="#5a6170" stroke-width="2"/>
<rect x="200.5" y="108" width="3" height="10" rx="1.5" fill="#2a303b"/>
<circle cx="224" cy="142" r="2.5" fill="#ff9a3c"/>"##;
    svg(w, h, &s)
}

/// The right side, seen from the right; its left edge meets the front.
pub fn side_right(p: &Palette) -> String {
    let [_, h, d] = BODY_SVG;
    let mut s = format!(
        r#"<defs>{}</defs><rect width="{d}" height="{h}" fill="url(#body)"/>"#,
        body_gradient(p)
    );
    for i in 0..6 {
        let x = 12.0 + 6.0 * i as f32;
        s += &format!(
            r##"<line x1="{x}" y1="36" x2="{x}" y2="66" stroke="#2a2e38" stroke-width="2" stroke-linecap="round" opacity="0.7"/>"##
        );
    }
    svg(d, h, &s)
}

/// The back, seen from behind: vent slots and a dark rating plate, so a
/// tumbling body still reads as a microwave from behind.
pub fn back(p: &Palette) -> String {
    let [w, h, _] = BODY_SVG;
    let mut s = format!(
        r#"<defs>{}</defs><rect width="{w}" height="{h}" fill="url(#body)"/>"#,
        body_gradient(p)
    );
    for row in 0..4 {
        for col in 0..10 {
            let x = 20.0 + 9.0 * col as f32;
            let y = 22.0 + 9.0 * row as f32;
            s += &format!(
                r##"<rect x="{x}" y="{y}" width="5" height="5" rx="1" fill="#2a2e38" opacity="0.7"/>"##
            );
        }
    }
    s += r##"<rect x="150" y="110" width="60" height="28" rx="3" fill="#2a2e38" opacity="0.55"/>"##;
    svg(w, h, &s)
}

/// Faces without details (left, top, bottom): the body gradient.
pub fn plain(p: &Palette) -> String {
    svg(
        64.0,
        64.0,
        &format!(
            r#"<defs>{}</defs><rect width="64" height="64" fill="url(#body)"/>"#,
            body_gradient(p)
        ),
    )
}

/// The frame of the door with a rounded window hole, as one even-odd path.
fn door_frame(fill: &str) -> String {
    let [_, _, w, h] = DOOR_RECT;
    // Window: x 14..142, y 14..126, radius 8 (the 2D door sprite).
    format!(
        r#"<path fill-rule="evenodd" fill="{fill}" d="M0 0H{w}V{h}H0Z M22 14H134A8 8 0 0 1 142 22V118A8 8 0 0 1 134 126H22A8 8 0 0 1 14 118V22A8 8 0 0 1 22 14Z"/>"#
    )
}

/// The outside of the door: dark frame, tinted glass, shine and handle.
/// The glass is translucent, so the lit cavity shows through it.
pub fn door_outer() -> String {
    let [_, _, w, h] = DOOR_RECT;
    let s = format!(
        r##"<defs><linearGradient id="glass" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#1b2433"/><stop offset="1" stop-color="#0b0f17"/></linearGradient>
<linearGradient id="shine" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffffff" stop-opacity="0.35"/><stop offset="0.45" stop-color="#ffffff" stop-opacity="0"/></linearGradient></defs>
{}
<rect x="0.5" y="0.5" width="{}" height="{}" rx="1" fill="none" stroke="#111111" stroke-width="1"/>
<rect x="14" y="14" width="128" height="112" rx="8" fill="url(#glass)" opacity="0.5"/>
<rect x="14" y="14" width="128" height="112" rx="8" fill="url(#shine)"/>
<rect x="144" y="40" width="5" height="60" rx="2.5" fill="#d8dce4"/>"##,
        door_frame("#262c37"),
        w - 1.0,
        h - 1.0
    );
    svg(w, h, &s)
}

/// The inside of the door, seen from the cavity side.
pub fn door_inner() -> String {
    let [_, _, w, h] = DOOR_RECT;
    let s = format!(
        r##"{}
<rect x="5" y="5" width="{}" height="{}" rx="4" fill="none" stroke="#3a404c" stroke-width="2"/>
<rect x="14" y="14" width="128" height="112" rx="8" fill="#121821" opacity="0.45"/>"##,
        door_frame("#1d222b"),
        w - 10.0,
        h - 10.0
    );
    svg(w, h, &s)
}

/// The edges of the door: one dark color.
pub fn door_edge() -> String {
    svg(
        16.0,
        16.0,
        r##"<rect width="16" height="16" fill="#1d222b"/>"##,
    )
}

/// The lit cavity walls (the glow of the 2D sprite).
pub fn cavity() -> String {
    svg(
        128.0,
        128.0,
        r##"<defs><radialGradient id="glow" cx="0.5" cy="0.4" r="0.75"><stop offset="0" stop-color="#ffd27a"/><stop offset="0.55" stop-color="#ff9a3c"/><stop offset="1" stop-color="#7a3410"/></radialGradient></defs><rect width="128" height="128" fill="url(#glow)"/>"##,
    )
}

/// The turntable plate, seen from above. Opaque everywhere, so the opaque
/// pass never discards the rim; the marks show that it turns.
pub fn turntable() -> String {
    let mut s = String::from(
        r##"<rect width="256" height="256" fill="#e8a060"/>
<circle cx="128" cy="128" r="124" fill="#fff4dc" stroke="#c46a26" stroke-width="5"/>
<circle cx="128" cy="128" r="84" fill="none" stroke="#e0c49a" stroke-width="3"/>"##,
    );
    for i in 0..3 {
        let a = i as f32 * 120.0;
        s += &format!(
            r##"<rect x="122" y="10" width="12" height="40" rx="6" fill="#d9a46a" transform="rotate({a} 128 128)"/>"##
        );
    }
    svg(256.0, 256.0, &s)
}

/// Segments a, b, c, d, e, f, g of the digits 0 to 9.
const SEGMENTS: [[bool; 7]; 10] = [
    [true, true, true, true, true, true, false],
    [false, true, true, false, false, false, false],
    [true, true, false, true, true, false, true],
    [true, true, true, true, false, false, true],
    [false, true, true, false, false, true, true],
    [true, false, true, true, false, true, true],
    [true, false, true, true, true, true, true],
    [true, true, true, false, false, false, false],
    [true; 7],
    [true, true, true, true, false, true, true],
];

/// The LED window with a fixed "12:34" in seven-segment style, drawn with
/// rectangles only (resvg is built without text). Same geometry as the
/// 2D sprite: digits 8 units wide, unlit segments at 8% opacity.
pub fn led(text: &str) -> String {
    let [_, _, w, h] = LED_RECT;
    let mut s = format!(r##"<rect width="{w}" height="{h}" fill="#06110f"/>"##);
    let y0 = 4.0;
    let mut x0 = 3.0;
    for ch in text.chars() {
        if ch == ':' {
            for y in [7.5, 12.5] {
                s += &format!(r##"<rect x="{x0}" y="{y}" width="2" height="2" fill="#5ee7df"/>"##);
            }
            x0 += 4.0;
            continue;
        }
        let digit = ch.to_digit(10).unwrap_or(8) as usize;
        let rects = [
            (x0 + 1.0, y0, 6.0, 2.0),
            (x0 + 6.0, y0 + 2.0, 2.0, 5.0),
            (x0 + 6.0, y0 + 7.0, 2.0, 5.0),
            (x0 + 1.0, y0 + 12.0, 6.0, 2.0),
            (x0, y0 + 7.0, 2.0, 5.0),
            (x0, y0 + 2.0, 2.0, 5.0),
            (x0 + 1.0, y0 + 6.0, 6.0, 2.0),
        ];
        for ((x, y, rw, rh), lit) in rects.into_iter().zip(SEGMENTS[digit]) {
            let opacity = if lit { 1.0 } else { 0.08 };
            s += &format!(
                r##"<rect x="{x}" y="{y}" width="{rw}" height="{rh}" fill="#5ee7df" opacity="{opacity}"/>"##
            );
        }
        x0 += 10.0;
    }
    svg(w, h, &s)
}
