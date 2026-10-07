use crate::annif::ProcessResult;
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use rfd::FileDialog;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct VocabulariesUi {
    vocab_id: String,
    vocab_path: String,
    language: String,
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
            language: "pt".into(),
            logs: Arc::new(Mutex::new(Vec::new())),
            is_loading: Arc::new(Mutex::new(false)),
            result: Arc::new(Mutex::new(None)),
            log_receiver: None,
        }
    }
}

impl VocabulariesUi {
    pub fn is_busy(&self) -> bool {
        *self.is_loading.lock().unwrap()
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, runtime: &ApplicationRuntime) {
        ui.heading("Gerenciar Vocabulários");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("ID do Vocabulário (ex: nalt):");
            ui.text_edit_singleline(&mut self.vocab_id);
        });

        ui.horizontal(|ui| {
            ui.label("Idioma (TSV):");
            ui.text_edit_singleline(&mut self.language);
        });
        ui.label("O ID deve constar no campo vocab de um projeto. TSV: URI e descritor separados por tabulação.");
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
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
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
                    ui.colored_label(
                        egui::Color32::from_rgb(21, 105, 46),
                        "Vocabulário carregado com sucesso!",
                    );
                    ui.label(format!("Duração: {} ms", p_res.duration.as_millis()));
                }
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Erro: {}", e));
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
        let language = self.language.clone();

        let is_loading_clone = self.is_loading.clone();
        let result_clone = self.result.clone();

        thread::spawn(move || {
            match crate::services::vocabularies::start(
                &runtime_clone,
                &vocab_id,
                &vocab_path,
                &language,
                tx,
            ) {
                Ok(handle) => {
                    let join_res = handle
                        .join()
                        .unwrap_or_else(|_| Err(anyhow::anyhow!("Thread panicked")));
                    *result_clone.lock().unwrap() = Some(join_res.map_err(|e| format!("{e:#}")));
                }
                Err(e) => {
                    *result_clone.lock().unwrap() = Some(Err(format!("{e:#}")));
                }
            }
            *is_loading_clone.lock().unwrap() = false;
        });
    }
}
