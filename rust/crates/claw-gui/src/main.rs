#![windows_subsystem = "windows"]

mod app;
mod backend;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("VIBE Paper — 医学科研助手"),
        ..Default::default()
    };

    eframe::run_native(
        "VIBE Paper",
        options,
        Box::new(|cc| Ok(Box::new(app::ClawMdApp::new(cc)))),
    )
}
