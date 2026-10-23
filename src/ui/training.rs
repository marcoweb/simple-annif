use crate::annif::{Cancellation, ProcessResult, Project};
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct TrainingUi {
    timer: Option<super::progress::ActivityTimer>,
    phase: &'static str,
    cancellation: Cancellation,
    result_project: Option<String>,
    data_path: String,
    logs: Arc<Mutex<Vec<String>>>,
    is_training: Arc<Mutex<bool>>,
    result: Arc<Mutex<Option<Result<ProcessResult, String>>>>,
    log_receiver: Option<crossbeam_channel::Receiver<String>>,
}

impl Default for TrainingUi {
    fn default() -> Self {
        Self {
            timer: None,
            phase: "Preparando operação",
            cancellation: Cancellation::default(),
            result_project: None,
            data_path: "".to_string(),
            logs: Arc::new(Mutex::new(Vec::new())),
            is_training: Arc::new(Mutex::new(false)),
            result: Arc::new(Mutex::new(None)),
            log_receiver: None,
        }
    }
}

impl TrainingUi {
    pub fn poll_results(&mut self) {
        if let Some(rx) = &self.log_receiver {
            while let Ok(log) = rx.try_recv() {
                if let Some(phase) = super::progress::phase_from_log(&log) {
                    self.phase = phase;
                }
                self.logs.lock().unwrap().push(log);
            }
        }
    }
    pub fn is_busy(&self) -> bool {
        *self.is_training.lock().unwrap()
    }
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
        if selected_project
            .as_ref()
            .is_some_and(|p| p.backend == "ensemble")
        {
            ui.label(
                "Ensemble combina modelos já treinados. Treine seus projetos fonte e use Suggest.",
            );
            return;
        }
        ui.label("Corpus UTF-8: pasta com pares .txt/.key ou arquivo TSV aceito pelo Annif.");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Corpus de treinamento:");
            super::text_input(ui, &mut self.data_path);
        });
        ui.horizontal(|ui| {
            if ui.button("Selecionar pasta...").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                self.data_path = path.display().to_string();
            }
            if ui.button("Selecionar arquivo...").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_file()
            {
                self.data_path = path.display().to_string();
            }
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
            super::progress::show_indeterminate(ui, self.phase, self.cancellation.is_requested());
            super::cancel_button(ui, &self.cancellation);
            ui.small("Interromper pode deixar o modelo incompleto. Execute o treinamento novamente antes de usar Suggest.");
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Treinando...");
            });
        }

        if let Some(timer) = &self.timer {
            timer.show(ui);
        }
        if let Some(project) = &self.result_project {
            ui.label(format!("Logs e resultado do projeto: {project}"));
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
                Ok(p_res) if p_res.cancelled => {
                    ui.label("Treinamento cancelado. Os logs foram preservados. Treine o projeto novamente antes de usar o modelo; arquivos parciais podem ter sido gravados.");
                }
                Ok(p_res) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(21, 105, 46),
                        "Treinamento concluído!",
                    );
                    ui.label(format!("Duração: {} ms", p_res.duration.as_millis()));
                }
                Err(e) => {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("Erro no treinamento: {}", e),
                    );
                }
            }
        }
    }

    fn start_training(&mut self, project_id: &str, runtime: &ApplicationRuntime) {
        let timer = super::progress::ActivityTimer::start();
        self.timer = Some(timer.clone());
        self.phase = "Preparando operação";
        self.result_project = Some(project_id.to_owned());
        *self.is_training.lock().unwrap() = true;
        self.logs.lock().unwrap().clear();
        *self.result.lock().unwrap() = None;

        let (tx, rx) = crossbeam_channel::unbounded();
        self.log_receiver = Some(rx);

        self.cancellation = Cancellation::default();
        let runtime_clone = runtime.with_cancellation(self.cancellation.clone());
        let project_id = project_id.to_string();
        let data_path = self.data_path.clone();

        let is_training_clone = self.is_training.clone();
        let result_clone = self.result.clone();

        thread::spawn(move || {
            match crate::services::training::start(&runtime_clone, &project_id, &data_path, tx) {
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
            timer.finish();
            *is_training_clone.lock().unwrap() = false;
        });
    }
}
