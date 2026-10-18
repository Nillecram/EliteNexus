//! The app icon, drawn in code (no asset files): an orange diamond with a light rim on a dark square, on a
//! 64-unit grid. build.rs renders it into the Windows .ico (it includes this file); the app uses it for its window
//! and, on Linux, for the app-menu icons (desktop_entry.rs).

/// The design at a point of the 64-unit grid: RGB 0-255, alpha 0-1.
fn design(u: f32, v: f32) -> [f32; 4] {
    let c = 31.5;
    let (dx, dy) = ((u - c).abs(), (v - c).abs());
    let d = dx + dy;
    if d < 20.0 {
        [255.0, 140.0, 26.0, 1.0]
    } else if d < 23.0 {
        [255.0, 190.0, 110.0, 1.0]
    } else if dx.max(dy) < 30.0 {
        [12.0, 17.0, 24.0, 1.0]
    } else {
        [0.0, 0.0, 0.0, 0.0]
    }
}

/// One pixel of an `n`×`n` rendering, 4×4 supersampled (RGBA).
pub fn pixel(n: u32, x: u32, y: u32) -> [u8; 4] {
    const S: u32 = 4;
    let scale = 64.0 / n as f32;
    let mut acc = [0.0f32; 4];
    for sy in 0..S {
        for sx in 0..S {
            let u = (x as f32 + (sx as f32 + 0.5) / S as f32) * scale;
            let v = (y as f32 + (sy as f32 + 0.5) / S as f32) * scale;
            let [r, g, b, a] = design(u, v);
            acc[0] += r * a;
            acc[1] += g * a;
            acc[2] += b * a;
            acc[3] += a;
        }
    }
    let k = (S * S) as f32;
    let alpha = acc[3] / k;
    if alpha <= 0.0 {
        return [0, 0, 0, 0];
    }
    // Un-premultiply.
    let ch = |i: usize| (acc[i] / acc[3]).round().clamp(0.0, 255.0) as u8;
    [ch(0), ch(1), ch(2), (alpha * 255.0).round() as u8]
}

/// An `n`×`n` RGBA rendering, rows top to bottom.
pub fn rgba(n: u32) -> Vec<u8> {
    (0..n).flat_map(|y| (0..n).flat_map(move |x| pixel(n, x, y))).collect()
}
