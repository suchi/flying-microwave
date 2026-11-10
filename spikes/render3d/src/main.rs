//! Technical spike of the 3D rendering (plan 0005, task 4): textured box
//! meshes, a depth buffer, a perspective camera and a door turning about its
//! hinge, drawn offscreen with wgpu and written as PNG files.
//!
//! Usage: `cargo run --release --locked -- [OUTPUT_DIR]` (default `out`).
//! Throwaway code: not part of the product.

mod art;
mod atlas;
mod gpu;
mod math;
mod mesh;
mod scene;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use resvg::tiny_skia::{IntSize, Pixmap};

use scene::Style;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
/// Times of the tumbling images, in seconds.
const TUMBLING_TIMES: [f32; 4] = [0.0, 0.5, 1.0, 1.5];

fn save(path: &Path, width: u32, height: u32, pixels: Vec<u8>) -> Result<(), String> {
    let size = IntSize::from_wh(width, height).ok_or("empty image")?;
    // The pixels are opaque, so straight and premultiplied alpha agree.
    let pixmap = Pixmap::from_vec(pixels, size).ok_or("bad pixel buffer")?;
    pixmap
        .save_png(path)
        .map_err(|e| format!("writing {}: {e}", path.display()))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn run(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("creating {}: {e}", out.display()))?;

    let started = Instant::now();
    let (jobs, textures) = mesh::texture_jobs();
    let atlas = atlas::build(&jobs)?;
    println!(
        "atlas: {}x{} with {} mip levels, rasterized in {:.1} ms",
        atlas.width,
        atlas.height,
        atlas.levels.len(),
        ms(started.elapsed())
    );
    save(
        &out.join("atlas.png"),
        atlas.width,
        atlas.height,
        atlas.levels[0].clone(),
    )?;
    let colors: Vec<&str> = art::PALETTES.iter().map(|p| p.name).collect();
    println!("body colors: {}", colors.join(", "));
    let meshes = mesh::build(&atlas.uvs, &textures);
    println!(
        "meshes: {} vertices, {} triangles",
        meshes.vertices.len(),
        meshes.indices.len() / 3
    );

    let started = Instant::now();
    let renderer = gpu::Renderer::new(&atlas, &meshes)?;
    let info = &renderer.adapter_info;
    println!(
        "adapter: {} ({:?}, {:?}, driver {} {}), anisotropy {}, MSAA x{}",
        info.name,
        info.device_type,
        info.backend,
        info.driver,
        info.driver_info,
        renderer.anisotropy,
        gpu::SAMPLES
    );
    println!("GPU setup: {:.1} ms", ms(started.elapsed()));

    let aspect = WIDTH as f32 / HEIGHT as f32;
    let frame = scene::frame(aspect);
    let flights = scene::flights(aspect);
    let mut shots: Vec<(String, Vec<gpu::Flyer>)> = TUMBLING_TIMES
        .iter()
        .map(|&t| {
            (
                format!("tumbling-{t:.1}.png"),
                scene::flyers_at(&flights, Style::Tumbling, t),
            )
        })
        .collect();
    shots.push((
        "level.png".into(),
        scene::flyers_at(&flights, Style::Level, 0.0),
    ));
    shots.push((
        "level-facing.png".into(),
        scene::flyers_at(&flights, Style::LevelFacing, 0.0),
    ));
    for (name, yaw, pitch, door, distance) in scene::CLOSEUPS {
        shots.push((
            format!("closeup-{name}.png"),
            scene::closeup(yaw, pitch, door, distance),
        ));
    }

    for (name, flyers) in &shots {
        let (pixels, timing) = renderer.render(WIDTH, HEIGHT, &frame, flyers);
        let path = out.join(name);
        save(&path, WIDTH, HEIGHT, pixels)?;
        println!(
            "{}: draw {:.1} ms, readback {:.1} ms",
            path.display(),
            ms(timing.draw),
            ms(timing.readback)
        );
    }
    Ok(())
}

fn main() {
    let out = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("out"), PathBuf::from);
    if let Err(e) = run(&out) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
