use crate::annif::ProcessResult;
use crate::services::diagnostics::{self, DiagnosticEvent};
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

#[derive(Default)]
pub struct DiagnosticsUi {
    receiver: Option<Receiver<DiagnosticEvent>>,
    python_result: Option<Result<ProcessResult, String>>,
    annif_result: Option<Result<ProcessResult, String>>,
    error: Option<String>,
    workspace_input: Option<String>,
}

impl DiagnosticsUi {
    pub fn ui(&mut self, ui: &mut egui::Ui, runtime: &mut ApplicationRuntime) {
        self.poll_results();
        let is_checking = self.receiver.is_some();
        ui.heading("Diagnóstico de Ambiente");
        ui.label(format!(
            "Sistema: {} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        let configured = match runtime {
            ApplicationRuntime::System(system) => {
                ui.label("Runtime: sistema (desenvolvimento)");
                ui.add_enabled_ui(!is_checking, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Executável Python:");
                        ui.text_edit_singleline(&mut system.python_cmd);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Executável Annif:");
                        ui.text_edit_singleline(&mut system.annif_cmd);
                    });
                });
                ui.label(
                    "Informe um nome no PATH ou o caminho completo, sem aspas e sem argumentos.",
                );
                !system.python_cmd.trim().is_empty() && !system.annif_cmd.trim().is_empty()
            }
            ApplicationRuntime::Embedded(embedded) => {
                ui.label("Runtime: incorporado (portátil)");
                ui.label(format!("Python: {}", embedded.python().display()));
                ui.label("Python e Annif são fornecidos pelo pacote da aplicação.");
                ui.label(format!(
                    "Workspace atual: {}",
                    embedded.workspace().display()
                ));
                let workspace = self
                    .workspace_input
                    .get_or_insert_with(|| embedded.workspace().display().to_string());
                let mut apply = false;
                ui.add_enabled_ui(!is_checking, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Workspace:");
                        ui.text_edit_singleline(workspace);
                        apply = ui.button("Aplicar workspace").clicked();
                    });
                });
                ui.label("Informe um caminho absoluto. A configuração vale para esta sessão.");
                if apply {
                    match runtime.configure_workspace(Path::new(workspace)) {
                        Ok(()) => {
                            self.error = None;
                            self.python_result = None;
                            self.annif_result = None;
                        }
                        Err(error) => self.error = Some(format!("{error:#}")),
                    }
                }
                true
            }
            ApplicationRuntime::Unavailable(error) => {
                ui.label("Runtime: incorporado indisponível");
                ui.colored_label(ui.visuals().error_fg_color, error.as_str());
                false
            }
        };
        ui.add_space(10.0);
        if ui
            .add_enabled(
                !is_checking && configured,
                egui::Button::new("Verificar ambiente"),
            )
            .clicked()
        {
            self.python_result = None;
            self.annif_result = None;
            self.error = None;
            match diagnostics::check_environment(runtime.clone()) {
                Ok(receiver) => self.receiver = Some(receiver),
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if self.receiver.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Verificando...");
            });
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(result) = &self.python_result {
                ui.heading("Python");
                Self::show_result(ui, result);
            }
            if let Some(result) = &self.annif_result {
                ui.heading("Annif / versão");
                Self::show_result(ui, result);
            }
        });
    }

    fn poll_results(&mut self) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        loop {
            match receiver.try_recv() {
                Ok(DiagnosticEvent::Python(result)) => self.python_result = Some(result),
                Ok(DiagnosticEvent::Annif(result)) => self.annif_result = Some(result),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.python_result.is_none() || self.annif_result.is_none() {
                        self.error =
                            Some("A verificação foi interrompida antes de concluir.".into());
                    }
                    self.receiver = None;
                    break;
                }
            }
        }
    }

    fn show_result(ui: &mut egui::Ui, result: &Result<ProcessResult, String>) {
        match result {
            Ok(result) => {
                if result.exit_code == Some(0) {
                    ui.colored_label(egui::Color32::from_rgb(21, 105, 46), "OK");
                } else {
                    ui.colored_label(ui.visuals().error_fg_color, "Falha na verificação");
                }
                ui.label(format!("Comando: {}", result.command));
                ui.label(format!("Duração: {} ms", result.duration.as_millis()));
                ui.label(format!(
                    "Código de saída: {}",
                    result
                        .exit_code
                        .map_or_else(|| "Indisponível".into(), |code| code.to_string())
                ));
                ui.label("Stdout:");
                ui.code(if result.stdout.is_empty() {
                    "(vazio)"
                } else {
                    &result.stdout
                });
                ui.label("Stderr:");
                ui.code(if result.stderr.is_empty() {
                    "(vazio)"
                } else {
                    &result.stderr
                });
            }
            Err(error) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("Erro: {error}"));
            }
        }
        ui.add_space(10.0);
    }
}
