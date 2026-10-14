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
    vocabularies: Vec<crate::annif::Vocabulary>,
    list_receiver:
        Option<crossbeam_channel::Receiver<Result<Vec<crate::annif::Vocabulary>, String>>>,
    list_error: Option<String>,
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
            vocabularies: Vec::new(),
            list_receiver: None,
            list_error: None,
        }
    }
}

impl VocabulariesUi {
    #[cfg(test)]
    pub(crate) fn completed_for_test() -> Self {
        let (tx, rx) = crossbeam_channel::bounded(1);
        tx.send(Ok(Vec::new())).unwrap();
        Self {
            list_receiver: Some(rx),
            ..Default::default()
        }
    }

    pub fn is_busy(&self) -> bool {
        *self.is_loading.lock().unwrap() || self.list_receiver.is_some()
    }
    pub fn poll_results(&mut self) {
        if let Some(rx) = &self.list_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.list_receiver = None;
                    match result {
                        Ok(items) => {
                            self.vocabularies = items;
                            if !self.vocabularies.iter().any(|v| v.id == self.vocab_id) {
                                self.vocab_id = self
                                    .vocabularies
                                    .first()
                                    .map(|v| v.id.clone())
                                    .unwrap_or_default();
                            }
                        }
                        Err(error) => self.list_error = Some(error),
                    }
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.list_receiver = None;
                    self.list_error = Some(
                        "A consulta de vocabulários foi interrompida. Tente atualizar.".into(),
                    );
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {}
            }
        }
        if let Some(rx) = &self.log_receiver {
            while let Ok(log) = rx.try_recv() {
                self.logs.lock().unwrap().push(log);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, runtime: &ApplicationRuntime) {
        ui.heading("Gerenciar Vocabulários");
        ui.add_space(10.0);
        if self.list_receiver.is_some() {
            ui.spinner();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        if ui
            .add_enabled(!self.is_busy(), egui::Button::new("Atualizar vocabulários"))
            .clicked()
        {
            self.refresh(runtime);
        }
        if let Some(error) = &self.list_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let busy = self.is_busy();
        ui.add_enabled_ui(!busy, |ui| {
        egui::ComboBox::from_label("Vocabulário configurado")
            .selected_text(if self.vocab_id.is_empty() { "Atualize a lista" } else { &self.vocab_id })
            .show_ui(ui, |ui| {
                for vocab in &self.vocabularies {
                    ui.selectable_value(&mut self.vocab_id, vocab.id.clone(), &vocab.id);
                }
            });
        if let Some(vocab) = self.vocabularies.iter().find(|v| v.id == self.vocab_id) {
            if vocab.loaded {
                ui.label(format!("Última consulta: carregado, {} assuntos. Idiomas: {}", vocab.size.unwrap_or_default(), vocab.languages.join(", ")));
            } else {
                ui.label("Última consulta: vocabulário ainda não carregado.");
            }
        }
        ui.horizontal(|ui| {
            ui.label("Idioma (TSV):");
            super::text_input(ui, &mut self.language);
        });
        ui.label("Salve um projeto com o campo vocab na aba Projetos e atualize esta lista. TSV: URI e descritor separados por tabulação. RDF: os idiomas vêm do arquivo.");
        ui.add_space(10.0);
        });

        ui.horizontal(|ui| {
            ui.label("Arquivo de Vocabulário:");
            super::text_input(ui, &mut self.vocab_path);
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
                !self.is_busy()
                    && !self.vocab_id.is_empty()
                    && !self.vocab_path.is_empty()
                    && !self.language.trim().is_empty(),
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
                    ui.label("Clique em Atualizar vocabulários para consultar os assuntos e idiomas carregados.");
                }
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, "Não foi possível carregar o vocabulário. Confira os logs e os detalhes abaixo.");
                    egui::ScrollArea::vertical()
                        .id_salt("vocab_error")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            ui.label(e);
                        });
                }
            }
        }
    }

    fn refresh(&mut self, runtime: &ApplicationRuntime) {
        self.list_error = None;
        self.vocabularies.clear();
        self.vocab_id.clear();
        let runtime = runtime.clone();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.list_receiver = Some(rx);
        thread::spawn(move || {
            let _ = tx
                .send(crate::services::vocabularies::list(&runtime).map_err(|e| format!("{e:#}")));
        });
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
