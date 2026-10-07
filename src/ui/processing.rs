use crate::annif::Project;
use crate::services::{
    processing::{self, DocumentResult},
    runtime::ApplicationRuntime,
};
use eframe::egui;
use rfd::FileDialog;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

enum Event {
    Result(DocumentResult),
    Error(String),
}
#[derive(Default)]
pub struct ProcessingUi {
    paths_to_process: Vec<String>,
    receiver: Option<Receiver<Event>>,
    results: Vec<DocumentResult>,
    status: Option<String>,
}

impl ProcessingUi {
    pub fn is_busy(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
        if let Some(rx) = &self.receiver {
            loop {
                match rx.try_recv() {
                    Ok(Event::Result(result)) => self.results.push(result),
                    Ok(Event::Error(error)) => self.status = Some(error),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.receiver = None;
                        break;
                    }
                }
            }
        }
        ui.heading("Sugestões de assuntos");
        let Some(project) = selected_project else {
            ui.label("Selecione um projeto na aba Projetos.");
            return;
        };
        ui.label(format!(
            "Projeto: {} ({})",
            project.name, project.project_id
        ));
        ui.label("Arquivos de texto UTF-8. Para diretórios, serão usados somente os arquivos .txt da pasta.");
        ui.add_enabled_ui(!self.is_busy(), |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Selecionar arquivos...").clicked()
                    && let Some(files) =
                        FileDialog::new().add_filter("Texto", &["txt"]).pick_files()
                {
                    self.paths_to_process =
                        files.into_iter().map(|p| p.display().to_string()).collect();
                }
                if ui.button("Selecionar diretório...").clicked()
                    && let Some(path) = FileDialog::new().pick_folder()
                {
                    self.paths_to_process = vec![path.display().to_string()];
                }
                if ui.button("Limpar seleção").clicked() {
                    self.paths_to_process.clear();
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("suggest-inputs")
                .max_height(90.0)
                .show(ui, |ui| {
                    for path in &self.paths_to_process {
                        ui.label(path);
                    }
                });
            if ui
                .add_enabled(
                    !self.paths_to_process.is_empty(),
                    egui::Button::new("Executar Suggest"),
                )
                .clicked()
            {
                self.start(project.project_id.clone(), runtime.clone());
            }
        });
        if self.is_busy() {
            ui.spinner();
            ui.label(format!(
                "Processando… {} documento(s) concluído(s)",
                self.results.len()
            ));
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if !self.results.is_empty()
            && ui
                .add_enabled(
                    !self.is_busy(),
                    egui::Button::new("Exportar resultados (JSON)"),
                )
                .clicked()
            && let Some(path) = FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_file_name("sugestoes.json")
                .save_file()
        {
            self.status = Some(match processing::export(&path, &self.results) {
                Ok(()) => format!("Resultados salvos em {}", path.display()),
                Err(error) => format!("Erro ao exportar: {error:#}"),
            });
        }
        if let Some(status) = &self.status {
            ui.label(status);
        }
        egui::ScrollArea::both()
            .id_salt("suggest-results")
            .show(ui, |ui| {
                for (index, result) in self.results.iter().enumerate() {
                    ui.separator();
                    ui.label(&result.path);
                    if let Some(error) = &result.error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    } else {
                        ui.label(format!(
                            "{} sugestão(ões) · {} ms",
                            result.suggestions.len(),
                            result.duration_ms
                        ));
                        egui::Grid::new(("suggestions", index))
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Assunto");
                                ui.strong("Score");
                                ui.strong("URI");
                                ui.end_row();
                                for hit in &result.suggestions {
                                    ui.label(&hit.label);
                                    ui.label(format!("{:.4}", hit.score));
                                    ui.label(&hit.uri);
                                    ui.end_row();
                                }
                            });
                    }
                    ui.push_id(index, |ui| {
                        ui.collapsing("Saída completa", |ui| {
                            ui.code(&result.stdout);
                            ui.code(&result.stderr);
                        });
                    });
                }
            });
    }
    fn start(&mut self, project_id: String, runtime: ApplicationRuntime) {
        let (tx, rx) = mpsc::channel();
        let paths = self.paths_to_process.clone();
        self.status = None;
        self.results.clear();
        match std::thread::Builder::new()
            .name("annif-suggest".into())
            .spawn(move || match processing::input_files(&paths) {
                Ok(files) => {
                    for path in files {
                        if tx
                            .send(Event::Result(processing::suggest(
                                &runtime,
                                &project_id,
                                &path,
                            )))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                Err(error) => {
                    let _ = tx.send(Event::Error(format!("{error:#}")));
                }
            }) {
            Ok(_) => self.receiver = Some(rx),
            Err(error) => self.status = Some(format!("Não foi possível iniciar: {error}")),
        }
    }
}
