//! Elite Nexus — the native Elite Dangerous companion.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_tools;
mod app;
mod assistant;
mod core;
mod export;
mod finder;
mod jobs;
mod market;
mod pages;
mod secrets;
mod settings;
mod sync;
#[cfg(test)]
mod testutil;
mod theme;
mod tts;
mod untouched;
mod widgets;

fn app_icon() -> egui::IconData {
    // A 64×64 orange diamond on a dark rounded square, drawn procedurally (no asset files).
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let c = (n as f32 - 1.0) / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x as f32 - c).abs(), (y as f32 - c).abs());
            let i = (y * n + x) * 4;
            let in_square = dx.max(dy) < 30.0;
            let d = dx + dy;
            let (r, g, b, a) = if d < 20.0 {
                (255, 140, 26, 255)
            } else if d < 23.0 {
                (255, 190, 110, 255)
            } else if in_square {
                (12, 17, 24, 255)
            } else {
                (0, 0, 0, 0)
            };
            rgba[i..i + 4].copy_from_slice(&[r, g, b, a]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

/// Holds an exclusive lock on instance.lock for the life of the process. A second instance would
/// tail the same journal and double-upload to EDDN/EDSM/Inara, so it must not start.
fn single_instance(data_dir: &std::path::Path) -> Option<std::fs::File> {
    let path = data_dir.join("instance.lock");
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().create(true).truncate(false).write(true).share_mode(0).open(path).ok()
    }
    #[cfg(not(windows))]
    {
        std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path).ok()
    }
}

fn already_running_message() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
        let text: Vec<u16> = "Elite Nexus is already running.\0".encode_utf16().collect();
        let title: Vec<u16> = "Elite Nexus\0".encode_utf16().collect();
        // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
        }
    }
}

fn main() -> eframe::Result {
    let data_dir = settings::data_dir();
    let screenshot_run = std::env::args().any(|a| a == "--screenshot");
    let _instance = match single_instance(&data_dir) {
        Some(lock) => Some(lock),
        None if screenshot_run => None,
        None => {
            already_running_message();
            return Ok(());
        }
    };
    let log_file = std::fs::OpenOptions::new().create(true).append(true).open(data_dir.join("nexus.log")).ok();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,wgpu=warn,naga=warn,eframe=warn".into());
    match log_file {
        Some(f) => tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(std::sync::Mutex::new(f)).init(),
        None => tracing_subscriber::fmt().with_env_filter(filter).init(),
    }
    tracing::info!(version = env!("CARGO_PKG_VERSION"), data = %data_dir.display(), "Elite Nexus starting");
    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));

    // Optional: --page <name>  --screenshot <file.png>
    let args: Vec<String> = std::env::args().collect();
    let arg = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let start_page = arg("--page").and_then(|p| {
        pages::Page::GROUPS
            .iter()
            .flat_map(|(_, ps)| ps.iter())
            .find(|pg| pg.label().replace(' ', "").eq_ignore_ascii_case(&p.replace(' ', "")) || format!("{pg:?}").eq_ignore_ascii_case(&p))
            .copied()
    });
    let delay = std::time::Duration::from_secs_f64(arg("--shot-delay").and_then(|s| s.parse().ok()).unwrap_or(0.0));
    let shot = arg("--screenshot").map(|p| app::ScreenshotJob::new(p.into(), delay));
    let overrides = app::Overrides { journal_dir: arg("--journal-dir").map(Into::into), mute: args.iter().any(|a| a == "--mute"), tab: arg("--tab"), demo: args.iter().any(|a| a == "--demo") };
    let screenshot_mode = shot.is_some();

    // --size 1000x700: window size for responsive-layout screenshots.
    let size = arg("--size").and_then(|s| {
        let (w, h) = s.split_once('x')?;
        Some([w.parse::<f32>().ok()?, h.parse::<f32>().ok()?])
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Elite Nexus")
        .with_app_id("elite-nexus")
        .with_inner_size(size.unwrap_or([1480.0, 920.0]))
        .with_min_inner_size([880.0, 600.0])
        .with_icon(app_icon());
    if screenshot_mode {
        // Render far outside the desktop, unfocused and without a taskbar button: UI checks must
        // not pop up over the game.
        viewport = viewport.with_active(false).with_position([-20_000.0, -20_000.0]).with_taskbar(false);
    }
    let options = eframe::NativeOptions {
        viewport,
        // Screenshot runs keep their own state and never restore a window size (eframe would
        // otherwise fall back to a default folder on C: and reuse the last geometry).
        persist_window: !screenshot_mode,
        persistence_path: Some(data_dir.join(if screenshot_mode { "screenshot-ui-state.ron" } else { "ui-state.ron" })),
        ..Default::default()
    };
    eframe::run_native(
        "Elite Nexus",
        options,
        Box::new(move |cc| Ok(Box::new(app::NexusApp::new(cc, data_dir, start_page, shot, overrides)))),
    )
}
