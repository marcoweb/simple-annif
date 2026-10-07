use crate::annif::{AnnifRuntime, ProcessResult, Project};
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct TrainingUi {
    data_path: String,
    logs: Arc<Mutex<Vec<String>>>,
    is_training: Arc<Mutex<bool>>,
    result: Arc<Mutex<Option<Result<ProcessResult, String>>>>,
    log_receiver: Option<crossbeam_channel::Receiver<String>>,
}

impl Default for TrainingUi {
    fn default() -> Self {
        Self {
            data_path: "".to_string(),
            logs: Arc::new(Mutex::new(Vec::new())),
            is_training: Arc::new(Mutex::new(false)),
            result: Arc::new(Mutex::new(None)),
            log_receiver: None,
        }
    }
}

impl TrainingUi {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
        ui.heading("Treinamento");
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
            ui.label("Diretório de Treinamento:");
            ui.text_edit_singleline(&mut self.data_path);
        });

        ui.add_space(10.0);

        let is_training = *self.is_training.lock().unwrap();

        if ui
            .add_enabled(
                !is_training && !self.data_path.is_empty(),
                egui::Button::new("Iniciar Treinamento"),
            )
            .clicked()
        {
            self.start_training(project_id, runtime);
        }

        ui.add_space(20.0);

        if is_training {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Treinando...");
            });
        }

        // Ler novos logs do channel
        if let Some(rx) = &self.log_receiver {
            while let Ok(log) = rx.try_recv() {
                self.logs.lock().unwrap().push(log);
            }
        }

        ui.heading("Logs");
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let logs = self.logs.lock().unwrap();
                for log in logs.iter() {
                    ui.label(log);
                }
            });

        ui.add_space(10.0);

        let res_lock = self.result.lock().unwrap().clone();
        if let Some(res) = res_lock {
            match res {
                Ok(p_res) => {
                    ui.colored_label(egui::Color32::GREEN, "Treinamento concluído!");
                    ui.label(format!("Duração: {} ms", p_res.duration.as_millis()));
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::RED, format!("Erro no treinamento: {}", e));
                }
            }
        }
    }

    fn start_training(&mut self, project_id: &str, runtime: &ApplicationRuntime) {
        *self.is_training.lock().unwrap() = true;
        self.logs.lock().unwrap().clear();
        *self.result.lock().unwrap() = None;

        let (tx, rx) = crossbeam_channel::unbounded();
        self.log_receiver = Some(rx);

        let runtime_clone = runtime.clone();
        let project_id = project_id.to_string();
        let data_path = self.data_path.clone();

        let is_training_clone = self.is_training.clone();
        let result_clone = self.result.clone();

        thread::spawn(move || {
            match runtime_clone.train(&project_id, &data_path, tx) {
                Ok(handle) => {
                    let join_res = handle
                        .join()
                        .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")));
                    *result_clone.lock().unwrap() = Some(join_res.map_err(|e| e.to_string()));
                }
                Err(e) => {
                    *result_clone.lock().unwrap() = Some(Err(e.to_string()));
                }
            }
            *is_training_clone.lock().unwrap() = false;
        });
    }
}
