use crate::annif::{AnnifRuntime, ProcessResult, Project};
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use rfd::FileDialog;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ProcessingUi {
    paths_to_process: Vec<String>,
    is_processing: Arc<Mutex<bool>>,
    results: Arc<Mutex<Vec<Result<ProcessResult, String>>>>,
}

impl Default for ProcessingUi {
    fn default() -> Self {
        Self {
            paths_to_process: Vec::new(),
            is_processing: Arc::new(Mutex::new(false)),
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ProcessingUi {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
        ui.heading("Processamento / Suggest");
        ui.add_space(10.0);

        let project_id = match selected_project {
            Some(p) => &p.project_id,
            None => {
                ui.label("Selecione um projeto na aba 'Projetos' primeiro.");
                return;
            }
        };

        ui.label(format!("Projeto Selecionado: {}", project_id));
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui.button("Selecionar Arquivos...").clicked()
                && let Some(files) = FileDialog::new().pick_files()
            {
                self.paths_to_process =
                    files.into_iter().map(|p| p.display().to_string()).collect();
            }
            if ui.button("Selecionar Diretório...").clicked()
                && let Some(folder) = FileDialog::new().pick_folder()
            {
                self.paths_to_process = vec![folder.display().to_string()];
            }
            if ui.button("Limpar Seleção").clicked() {
                self.paths_to_process.clear();
            }
        });

        ui.add_space(5.0);
        if !self.paths_to_process.is_empty() {
            ui.label(format!(
                "{} caminho(s) selecionado(s):",
                self.paths_to_process.len()
            ));
            egui::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for path in &self.paths_to_process {
                        ui.label(path);
                    }
                });
        } else {
            ui.label("Nenhum caminho selecionado.");
        }

        ui.add_space(10.0);

        let is_processing = *self.is_processing.lock().unwrap();

        if ui
            .add_enabled(
                !is_processing && !self.paths_to_process.is_empty(),
                egui::Button::new("Executar Suggest"),
            )
            .clicked()
        {
            self.start_suggest(project_id, runtime);
        }

        ui.add_space(20.0);

        if is_processing {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Processando...");
            });
        }

        let results_lock = self.results.lock().unwrap().clone();
        if !results_lock.is_empty() {
            ui.heading("Resultados");

            if ui.button("Exportar Resultados (JSON)").clicked()
                && let Some(save_path) = FileDialog::new().add_filter("JSON", &["json"]).save_file()
            {
                self.export_results(save_path.display().to_string(), &results_lock);
            }

            ui.add_space(5.0);

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for res in results_lock {
                        match res {
                            Ok(p_res) => {
                                ui.label(format!("Comando: {}", p_res.command));
                                ui.label(format!("Duração: {} ms", p_res.duration.as_millis()));
                                if !p_res.stdout.trim().is_empty() {
                                    ui.code(p_res.stdout.trim());
                                }
                                if !p_res.stderr.trim().is_empty() {
                                    ui.colored_label(egui::Color32::RED, "Stderr:");
                                    ui.code(p_res.stderr.trim());
                                }
                                ui.separator();
                            }
                            Err(e) => {
                                ui.colored_label(egui::Color32::RED, format!("Erro: {}", e));
                                ui.separator();
                            }
                        }
                    }
                });
        }
    }

    fn start_suggest(&mut self, project_id: &str, runtime: &ApplicationRuntime) {
        *self.is_processing.lock().unwrap() = true;
        self.results.lock().unwrap().clear();

        let runtime_clone = runtime.clone();
        let project_id = project_id.to_string();
        let paths = self.paths_to_process.clone();

        let is_processing_clone = self.is_processing.clone();
        let results_clone = self.results.clone();

        thread::spawn(move || {
            for path in paths {
                let res = runtime_clone
                    .suggest(&project_id, &path)
                    .map_err(|e| e.to_string());
                results_clone.lock().unwrap().push(res);
            }
            *is_processing_clone.lock().unwrap() = false;
        });
    }

    fn export_results(&self, save_path: String, results: &[Result<ProcessResult, String>]) {
        // Mapear os resultados para algo exportável e escrever em arquivo
        use serde_json::json;
        let mut export_data = Vec::new();

        for res in results {
            match res {
                Ok(p_res) => {
                    export_data.push(json!({
                        "command": p_res.command,
                        "stdout": p_res.stdout.trim(),
                        "stderr": p_res.stderr.trim(),
                        "exit_code": p_res.exit_code,
                        "duration_ms": p_res.duration.as_millis(),
                        "status": "success"
                    }));
                }
                Err(err) => {
                    export_data.push(json!({
                        "error": err,
                        "status": "error"
                    }));
                }
            }
        }

        if let Ok(json_str) = serde_json::to_string_pretty(&export_data) {
            let _ = std::fs::write(save_path, json_str);
        }
    }
}
