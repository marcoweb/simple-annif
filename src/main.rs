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
        #[cfg(windows)]
        wgpu_options: {
            let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
            // A enumeração de outros backends pode cair no driver OpenGL AMD
            // antes de abrir a janela. No Windows, inicialize somente DirectX 12.
            setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
            eframe::WgpuConfiguration {
                wgpu_setup: setup.into(),
                ..Default::default()
            }
        },
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Annif Manager",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::ThemePreference::Light);
            Ok(Box::new(AnnifManagerApp::default()))
        }),
    )
}
