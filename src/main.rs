#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod annif;
mod app;
mod services;
#[cfg(test)]
mod test_support;
mod ui;

use app::AnnifManagerApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Annif Manager",
        options,
        Box::new(|_cc| Ok(Box::new(AnnifManagerApp::default()))),
    )
}
