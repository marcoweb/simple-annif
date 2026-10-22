use crate::annif::{Cancellation, Project};
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
    cancellation: Cancellation,
    paths_to_process: Vec<String>,
    receiver: Option<Receiver<Event>>,
    results: Vec<DocumentResult>,
    status: Option<String>,
}

impl ProcessingUi {
    #[cfg(test)]
    pub(crate) fn completed_for_test() -> Self {
        let (tx, rx) = mpsc::channel();
        tx.send(Event::Error("erro de teste preservado".into()))
            .unwrap();
        Self {
            receiver: Some(rx),
            ..Default::default()
        }
    }

    pub fn is_busy(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn poll_results(&mut self) {
        if let Some(rx) = &self.receiver {
            loop {
                match rx.try_recv() {
                    Ok(Event::Result(result)) => self.results.push(result),
                    Ok(Event::Error(error)) => self.status = Some(error),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.receiver = None;
                        let cancelled = self
                            .results
                            .iter()
                            .filter(|result| result.cancelled)
                            .count();
                        if cancelled > 0 {
                            self.status = Some(format!(
                                "Processamento cancelado: {cancelled} documento(s) cancelado(s) ou não processado(s). Resultados anteriores preservados; exporte JSON para manter o registro completo."
                            ));
                        }
                        break;
                    }
                }
            }
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
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
            super::cancel_button(ui, &self.cancellation);
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
        if !self.results.is_empty() {
            if ui
                .add_enabled(
                    !self.is_busy(),
                    egui::Button::new("Exportar para Evametrics (CSV)"),
                )
                .clicked()
            {
                match crate::services::evametrics::render(&self.results) {
                    Err(error) => self.status = Some(format!("Erro ao exportar: {error:#}")),
                    Ok(_) => {
                        if let Some(path) = FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .set_file_name("sugestoes-evametrics.csv")
                            .save_file()
                        {
                            self.status = Some(
                                match crate::services::evametrics::export(&path, &self.results) {
                                    Ok(()) => format!(
                                        "Arquivo Evametrics salvo em {}. Confira se os IDs correspondem ao padrão-ouro.",
                                        path.display()
                                    ),
                                    Err(error) => format!("Erro ao exportar: {error:#}"),
                                },
                            );
                        }
                    }
                }
            }
            ui.small("Evametrics: IDs pelo nome do arquivo (D8001.txt → D8001; 1.txt → D1). Exporta descritores, sem scores; o título usa o nome do arquivo.");
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
                    ui.label(format!("Projeto do resultado: {}", result.project_id));
                    if let Some(error) = &result.error {
                        if result.cancelled {
                            ui.label(error);
                        } else {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
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
        self.cancellation = Cancellation::default();
        let cancellation = self.cancellation.clone();
        let runtime = runtime.with_cancellation(cancellation.clone());
        match std::thread::Builder::new()
            .name("annif-suggest".into())
            .spawn(move || match processing::input_files(&paths) {
                Ok(files) => {
                    processing::suggest_batch(
                        &runtime,
                        &project_id,
                        &files,
                        &cancellation,
                        |result| tx.send(Event::Result(result)).is_ok(),
                    );
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
