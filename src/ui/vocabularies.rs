use crate::annif::{AnnifRuntime, ProcessResult};
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use rfd::FileDialog;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct VocabulariesUi {
    vocab_id: String,
    vocab_path: String,
    logs: Arc<Mutex<Vec<String>>>,
    is_loading: Arc<Mutex<bool>>,
    result: Arc<Mutex<Option<Result<ProcessResult, String>>>>,
    log_receiver: Option<crossbeam_channel::Receiver<String>>,
}

impl Default for VocabulariesUi {
    fn default() -> Self {
        Self {
            vocab_id: "".to_string(),
            vocab_path: "".to_string(),
            logs: Arc::new(Mutex::new(Vec::new())),
            is_loading: Arc::new(Mutex::new(false)),
            result: Arc::new(Mutex::new(None)),
            log_receiver: None,
        }
    }
}

impl VocabulariesUi {
    pub fn ui(&mut self, ui: &mut egui::Ui, runtime: &ApplicationRuntime) {
        ui.heading("Gerenciar Vocabulários");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("ID do Vocabulário (ex: nalt):");
            ui.text_edit_singleline(&mut self.vocab_id);
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Arquivo de Vocabulário:");
            ui.text_edit_singleline(&mut self.vocab_path);
            if ui.button("Procurar...").clicked()
                && let Some(path) = FileDialog::new().pick_file()
            {
                self.vocab_path = path.display().to_string();
            }
        });

        ui.add_space(10.0);

        let is_loading = *self.is_loading.lock().unwrap();

        if ui
            .add_enabled(
                !is_loading && !self.vocab_id.is_empty() && !self.vocab_path.is_empty(),
                egui::Button::new("Carregar Vocabulário"),
            )
            .clicked()
        {
            self.load_vocab(runtime);
        }

        ui.add_space(20.0);

        if is_loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Carregando...");
            });
        }

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
                    ui.colored_label(egui::Color32::GREEN, "Vocabulário carregado com sucesso!");
                    ui.label(format!("Duração: {} ms", p_res.duration.as_millis()));
                }
                Err(e) => {
                    ui.colored_label(egui::Color32::RED, format!("Erro: {}", e));
                }
            }
        }
    }

    fn load_vocab(&mut self, runtime: &ApplicationRuntime) {
        *self.is_loading.lock().unwrap() = true;
        self.logs.lock().unwrap().clear();
        *self.result.lock().unwrap() = None;

        let (tx, rx) = crossbeam_channel::unbounded();
        self.log_receiver = Some(rx);

        let runtime_clone = runtime.clone();
        let vocab_id = self.vocab_id.clone();
        let vocab_path = self.vocab_path.clone();

        let is_loading_clone = self.is_loading.clone();
        let result_clone = self.result.clone();

        thread::spawn(move || {
            match runtime_clone.load_vocab(&vocab_id, &vocab_path, tx) {
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
            *is_loading_clone.lock().unwrap() = false;
        });
    }
}
