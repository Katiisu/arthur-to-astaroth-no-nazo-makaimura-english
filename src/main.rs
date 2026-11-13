// Hide the console window for the release .exe (the --cli mode re-attaches to the parent console).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod music;
mod patcher;

use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, RichText, Stroke, Vec2};
use patcher::{Checked, Track1State};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const GOLD: Color32 = Color32::from_rgb(0xf2, 0xb1, 0x3d);
const GHOST: Color32 = Color32::from_rgb(0x8e, 0xf0, 0xb4);
const PANEL: Color32 = Color32::from_rgba_premultiplied(18, 10, 32, 225);
const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe0, 0xf5);
const DIM: Color32 = Color32::from_rgb(0xa8, 0x9c, 0xc0);
const WARN: Color32 = Color32::from_rgb(0xff, 0x9a, 0x5c);

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "--cli" || args[1] == "--write-music" || args[1] == "--help") {
        attach_console();
        std::process::exit(cli(&args[1..]));
    }

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Nazo Makaimura English Patcher")
        .with_inner_size([880.0, 520.0])
        .with_resizable(false)
        .with_drag_and_drop(true);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    // A .cue dragged onto the .exe icon arrives as the first argument.
    let preload = args.get(1).map(PathBuf::from).filter(|p| p.exists());
    eframe::run_native(
        "Nazo Makaimura English Patcher",
        options,
        Box::new(move |cc| {
            let mut app = App::new(cc);
            if let Some(p) = preload {
                app.load(&p);
            }
            Ok(Box::new(app))
        }),
    )
}

// ------------------------------------------------------------------ command line

fn cli(args: &[String]) -> i32 {
    match args[0].as_str() {
        "--write-music" if args.len() >= 2 => {
            let wav = music::wav_bytes(&music::render());
            match std::fs::write(&args[1], wav) {
                Ok(_) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        }
        "--cli" if args.len() >= 2 => {
            let input = PathBuf::from(&args[1]);
            let force = args.iter().any(|a| a == "--force");
            let checked = match patcher::check(&input) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: {e}");
                    return 1;
                }
            };
            println!("Track 01: {:?}", checked.state);
            let out = args
                .get(2)
                .filter(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or_else(|| patcher::default_out_dir(&checked.disc));
            let mut last = String::new();
            let res = patcher::build(&checked, &out, force, &mut |done, total, label| {
                if label != last {
                    println!("{label} ({}%)", done * 100 / total.max(1));
                    last = label.to_string();
                }
            });
            match res {
                Ok(cue) => {
                    println!("Done: {}", cue.display());
                    0
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    1
                }
            }
        }
        _ => {
            println!(
                "Nazo Makaimura English Patcher {}\n\n\
                 Run without arguments for the normal window, or:\n  \
                 --cli <game.cue> [output folder] [--force]\n  \
                 --write-music <file.wav>",
                patcher::PATCH_VERSION
            );
            0
        }
    }
}

#[cfg(windows)]
fn attach_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(pid: u32) -> i32;
    }
    unsafe {
        AttachConsole(0xFFFF_FFFF);
    }
}
#[cfg(not(windows))]
fn attach_console() {}

// ------------------------------------------------------------------ window

#[derive(Default)]
struct Job {
    done: u64,
    total: u64,
    label: String,
    result: Option<Result<PathBuf, String>>,
}

struct App {
    checked: Option<Checked>,
    error: Option<String>,
    out_dir: Option<PathBuf>,
    allow_unknown: bool,
    job: Option<Arc<Mutex<Job>>>,
    music: music::Music,
    scene: egui::TextureHandle,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.all_styles_mut(|style| {
            style.visuals.override_text_color = Some(TEXT);
            style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x3b, 0x22, 0x5c);
            style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x52, 0x2f, 0x80);
            style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(0x6a, 0x3c, 0xa6);
            style.visuals.selection.bg_fill = Color32::from_rgb(0x8a, 0x4f, 0xd6);
            style.spacing.button_padding = Vec2::new(12.0, 6.0);
        });

        let mut music = music::Music::new();
        music.set(true);
        let scene = load_scene(&cc.egui_ctx);
        App {
            checked: None,
            error: None,
            out_dir: None,
            allow_unknown: false,
            job: None,
            music,
            scene,
        }
    }

    fn load(&mut self, path: &Path) {
        self.error = None;
        self.checked = None;
        self.job = None;
        self.allow_unknown = false;
        match patcher::check(path) {
            Ok(c) => {
                self.out_dir = Some(patcher::default_out_dir(&c.disc));
                self.checked = Some(c);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn start(&mut self, ctx: &egui::Context) {
        let (Some(checked), Some(out)) = (self.checked.clone(), self.out_dir.clone()) else {
            return;
        };
        let job = Arc::new(Mutex::new(Job {
            label: "Starting".into(),
            ..Default::default()
        }));
        self.job = Some(job.clone());
        let allow = self.allow_unknown;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let j2 = job.clone();
            let c2 = ctx.clone();
            let res = patcher::build(&checked, &out, allow, &mut |done, total, label| {
                let mut j = j2.lock().unwrap();
                j.done = done;
                j.total = total;
                if j.label != label {
                    j.label = label.to_string();
                }
                c2.request_repaint();
            });
            job.lock().unwrap().result = Some(res.map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
    }

    fn busy(&self) -> bool {
        self.job
            .as_ref()
            .map(|j| j.lock().unwrap().result.is_none())
            .unwrap_or(false)
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Files dropped onto the window
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if let Some(p) = dropped.first() {
            if !self.busy() {
                self.load(p);
            }
        }

        let t = ctx.input(|i| i.time) as f32;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                paint_scene(ui, &self.scene, t);
                let full = ui.max_rect();
                let card = egui::Rect::from_min_size(
                    Pos2::new(full.right() - 520.0, full.top() + 58.0),
                    Vec2::new(500.0, 400.0),
                );
                ui.painter()
                    .rect_filled(card, CornerRadius::same(14), PANEL);
                ui.painter().rect_stroke(
                    card,
                    CornerRadius::same(14),
                    Stroke::new(1.5, GOLD.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
                let inner = card.shrink2(Vec2::new(24.0, 18.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| self.card(ui));
                footer(ui, full, &mut self.music);
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

impl App {
    fn card(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(2.0);
            ui.label(
                RichText::new("Arthur & Astaroth's")
                    .font(FontId::proportional(16.0))
                    .color(DIM),
            );
            ui.label(
                RichText::new("Mysterious Ghosts 'n Goblins")
                    .font(FontId::proportional(30.0))
                    .color(GOLD)
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "Incredible Toons  ·  Unofficial English Patch {}",
                    patcher::PATCH_VERSION
                ))
                .font(FontId::proportional(15.0))
                .color(GHOST),
            );
        });
        ui.add_space(12.0);

        // Step 1: pick the disc
        let busy = self.busy();
        ui.label(RichText::new("1.  Choose your Japanese disc (Only require Track01").strong());
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Browse for .cue…"))
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Choose the Japanese game's .cue file")
                    .add_filter("CD image (.cue)", &["cue", "bin"])
                    .pick_file()
                {
                    self.load(&p);
                }
            }
            let shown = self
                .checked
                .as_ref()
                .map(|c| {
                    c.disc
                        .cue_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_else(|| "…or drag the .cue onto this window".into());
            ui.add(egui::Label::new(RichText::new(shown).color(DIM)).truncate());
        });

        if let Some(e) = &self.error {
            ui.add_space(4.0);
            ui.label(RichText::new(e).color(WARN));
        }

        if let Some(c) = &self.checked {
            ui.add_space(4.0);
            match &c.state {
                Track1State::Original => {
                    ui.label(
                        RichText::new("✔ Japanese original found. Ready to translate!")
                            .color(GHOST),
                    );
                }
                Track1State::AlreadyPatched => {
                    ui.label(
                        RichText::new("This disc is already in English. Nothing to do!")
                            .color(GOLD),
                    );
                }
                Track1State::Unknown(_) => {
                    ui.label(
                        RichText::new("⚠ This dump doesn't match the known Japanese disc. It may be a different version or a modified copy.")
                            .color(WARN),
                    );
                    ui.checkbox(
                        &mut self.allow_unknown,
                        "Patch anyway (advanced; the result may not work)",
                    );
                }
            }
        }

        // Step 2: output folder
        ui.add_space(12.0);
        ui.label(RichText::new("2.  Where to save the English copy").strong());
        ui.horizontal(|ui| {
            let can = self.checked.is_some() && !busy;
            if ui
                .add_enabled(can, egui::Button::new("Change folder…"))
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .set_title("Save the English copy in…")
                    .pick_folder()
                {
                    self.out_dir = Some(p);
                }
            }
            let shown = self
                .out_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into());
            ui.add(egui::Label::new(RichText::new(shown).color(DIM)).truncate());
        });
        ui.label(
            RichText::new("Your original files are never changed.")
                .small()
                .color(DIM),
        );

        // Step 3: go
        ui.add_space(14.0);
        let ready = match self.checked.as_ref().map(|c| &c.state) {
            Some(Track1State::Original) => true,
            Some(Track1State::Unknown(_)) => self.allow_unknown,
            _ => false,
        };
        ui.vertical_centered(|ui| {
            let btn = egui::Button::new(
                RichText::new("Translate to English!")
                    .font(FontId::proportional(20.0))
                    .strong(),
            )
            .fill(Color32::from_rgb(0x8a, 0x4f, 0xd6))
            .min_size(Vec2::new(260.0, 44.0));
            if ui.add_enabled(ready && !busy, btn).clicked() {
                let ctx = ui.ctx().clone();
                self.start(&ctx);
            }
        });

        // Progress / result
        if let Some(job) = &self.job {
            let j = job.lock().unwrap();
            ui.add_space(10.0);
            match &j.result {
                None => {
                    let frac = if j.total > 0 {
                        j.done as f32 / j.total as f32
                    } else {
                        0.0
                    };
                    ui.add(egui::ProgressBar::new(frac).text(format!(
                        "{}…  {:.0}%",
                        j.label,
                        frac * 100.0
                    )));
                }
                Some(Ok(cue)) => {
                    ui.label(
                        RichText::new("Done! Load this file in your emulator:")
                            .color(GHOST)
                            .strong(),
                    );
                    ui.label(RichText::new(cue.file_name().unwrap().to_string_lossy()).color(TEXT));
                    if ui.button("Open folder").clicked() {
                        open_folder(cue.parent().unwrap_or(Path::new(".")));
                    }
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(format!("Something went wrong: {e}")).color(WARN));
                }
            }
        }
    }
}

fn footer(ui: &mut egui::Ui, full: egui::Rect, music: &mut music::Music) {
    let r = egui::Rect::from_min_max(
        Pos2::new(full.left() + 14.0, full.bottom() - 34.0),
        Pos2::new(full.right() - 14.0, full.bottom() - 6.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
        ui.horizontal(|ui| {
            let label = if music.playing { "Music: On" } else { "Music: Off" };
            if ui.small_button(label).clicked() {
                let on = !music.playing;
                music.set(on);
            }
            ui.label(
                RichText::new("Fan translation · not affiliated with Capcom · you need your own copy of the game")
                    .small()
                    .color(DIM),
            );
        });
    });
}

fn open_folder(p: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

// ------------------------------------------------------------------ scenery (original pixel art + animated storm)

/// Art is 220x130 pixels, drawn at 4x.
const PX: f32 = 4.0;

fn load_scene(ctx: &egui::Context) -> egui::TextureHandle {
    let img = image::load_from_memory_with_format(
        include_bytes!("../assets/castle.png"),
        image::ImageFormat::Png,
    )
    .expect("castle.png")
    .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture("castle", color, egui::TextureOptions::NEAREST)
}

/// Small deterministic hash -> 0..1
fn rnd(k: u32) -> f32 {
    let mut x = k.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

fn paint_scene(ui: &egui::Ui, tex: &egui::TextureHandle, t: f32) {
    let r = ui.max_rect();
    let p = ui.painter();
    let uv = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    p.image(
        tex.id(),
        egui::Rect::from_min_size(r.min, Vec2::new(220.0 * PX, 130.0 * PX)),
        uv,
        Color32::WHITE,
    );

    // Lightning: one strike per ~7 s period, at a random moment, with a double flash.
    let period = 7.0;
    let n = (t / period).floor() as u32;
    let local = t - n as f32 * period;
    let strike_at = 1.5 + rnd(n * 3 + 1) * 4.0;
    let dt = local - strike_at;
    let flash = if (0.0..0.09).contains(&dt) {
        1.0
    } else if (0.16..0.30).contains(&dt) {
        0.7 * (1.0 - (dt - 0.16) / 0.14)
    } else {
        0.0
    };
    if flash > 0.0 {
        p.rect_filled(
            r,
            0.0,
            Color32::from_rgba_unmultiplied(220, 210, 255, (70.0 * flash) as u8),
        );
        // bolt, built from art pixels
        let xs = [12.0, 96.0, 84.0, 30.0];
        let mut x = xs[(rnd(n * 3 + 2) * 4.0) as usize % 4];
        let mut y = 6.0f32;
        let mut k = n * 97;
        let mut pts = vec![(x, y)];
        while y < 92.0 {
            k += 1;
            y += 3.0 + (rnd(k) * 5.0).floor();
            x += (rnd(k + 500) * 7.0).floor() - 3.0;
            pts.push((x, y));
        }
        for (col, w) in [
            (
                Color32::from_rgba_unmultiplied(170, 150, 255, (120.0 * flash) as u8),
                1.0,
            ),
            (
                Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * flash) as u8),
                0.0,
            ),
        ] {
            for seg in pts.windows(2) {
                let (x0, y0) = seg[0];
                let (x1, y1) = seg[1];
                let steps = (y1 - y0).max((x1 - x0).abs()).max(1.0) as i32;
                for i in 0..=steps {
                    let ax = (x0 + (x1 - x0) * i as f32 / steps as f32).round();
                    let ay = (y0 + (y1 - y0) * i as f32 / steps as f32).round();
                    let min = r.min + Vec2::new((ax - w) * PX, (ay - w) * PX);
                    p.rect_filled(
                        egui::Rect::from_min_size(min, Vec2::splat((1.0 + 2.0 * w) * PX)),
                        0.0,
                        col,
                    );
                }
            }
        }
    }

    // Rain: slanted streaks snapped to the art's pixel grid
    let rain = Color32::from_rgba_unmultiplied(150, 150, 205, 70);
    for i in 0..150u32 {
        let speed = 150.0 + rnd(i) * 70.0; // art pixels per second
        let y = (rnd(i + 1000) * 150.0 + t * speed) % 150.0 - 10.0;
        let x = (rnd(i + 2000) * 240.0 + t * speed * 0.25) % 240.0 - 10.0;
        let (gx, gy) = (x.floor(), y.floor());
        for k in 0..3 {
            let min = r.min
                + Vec2::new(
                    (gx - k as f32 * 0.0 - (k / 2) as f32) * PX,
                    (gy - k as f32) * PX,
                );
            p.rect_filled(egui::Rect::from_min_size(min, Vec2::new(PX, PX)), 0.0, rain);
        }
    }
}
