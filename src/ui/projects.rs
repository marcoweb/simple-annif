use crate::annif::{AnnifRuntime, Project};
use crate::services::runtime::ApplicationRuntime;
use eframe::egui;
use rfd::FileDialog;
use std::sync::{Arc, Mutex};
use std::thread;

type ProjectsResult = Option<Result<Vec<Project>, String>>;

pub struct ProjectsUi {
    projects: Arc<Mutex<ProjectsResult>>,
    is_loading: Arc<Mutex<bool>>,
    projects_toml_path: String,
    projects_toml_content: String,
    is_editing_toml: bool,
    save_status: Option<String>,
}

impl Default for ProjectsUi {
    fn default() -> Self {
        Self {
            projects: Arc::new(Mutex::new(None)),
            is_loading: Arc::new(Mutex::new(false)),
            projects_toml_path: "".to_string(),
            projects_toml_content: "".to_string(),
            is_editing_toml: false,
            save_status: None,
        }
    }
}

impl ProjectsUi {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &mut Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
        ui.heading("Projetos Annif");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui
                .selectable_label(!self.is_editing_toml, "Lista de Projetos")
                .clicked()
            {
                self.is_editing_toml = false;
            }
            if ui
                .selectable_label(self.is_editing_toml, "Editar projects.toml")
                .clicked()
            {
                self.is_editing_toml = true;
            }
        });

        ui.separator();

        if self.is_editing_toml {
            self.ui_edit_toml(ui);
        } else {
            self.ui_list_projects(ui, selected_project, runtime);
        }
    }

    fn ui_list_projects(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &mut Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
        let is_loading = *self.is_loading.lock().unwrap();

        if ui
            .add_enabled(!is_loading, egui::Button::new("Atualizar Lista via Annif"))
            .clicked()
        {
            self.load_projects(runtime);
        }

        ui.add_space(10.0);

        if is_loading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Carregando projetos...");
            });
            return;
        }

        let projects_res = self.projects.lock().unwrap().clone();

        match projects_res {
            Some(Ok(projects)) => {
                if projects.is_empty() {
                    ui.label("Nenhum projeto encontrado.");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for project in projects {
                                let is_selected = selected_project.as_ref() == Some(&project);
                                if ui
                                    .selectable_label(
                                        is_selected,
                                        format!("{} ({})", project.name, project.project_id),
                                    )
                                    .clicked()
                                {
                                    *selected_project = Some(project.clone());
                                }
                            }
                        });

                    if let Some(selected) = selected_project {
                        ui.separator();
                        ui.heading("Detalhes do Projeto");
                        ui.label(format!("ID: {}", selected.project_id));
                        ui.label(format!("Nome: {}", selected.name));
                        ui.label(format!("Backend: {}", selected.backend));
                    }
                }
            }
            Some(Err(err)) => {
                ui.colored_label(egui::Color32::RED, format!("Erro: {}", err));
            }
            None => {
                ui.label("Clique em 'Atualizar Lista' para buscar os projetos.");
            }
        }
    }

    fn ui_edit_toml(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Arquivo projects.toml:");
            ui.text_edit_singleline(&mut self.projects_toml_path);
            if ui.button("Procurar...").clicked()
                && let Some(path) = FileDialog::new().add_filter("TOML", &["toml"]).pick_file()
            {
                self.projects_toml_path = path.display().to_string();
                if let Ok(content) = std::fs::read_to_string(&self.projects_toml_path) {
                    self.projects_toml_content = content;
                    self.save_status = None;
                } else {
                    self.save_status = Some("Erro ao ler o arquivo.".to_string());
                }
            }
        });

        ui.add_space(10.0);

        if !self.projects_toml_path.is_empty() {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.projects_toml_content)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });

            ui.add_space(10.0);

            if ui.button("Salvar Alterações").clicked() {
                match std::fs::write(&self.projects_toml_path, &self.projects_toml_content) {
                    Ok(_) => self.save_status = Some("Salvo com sucesso!".to_string()),
                    Err(e) => self.save_status = Some(format!("Erro ao salvar: {}", e)),
                }
            }

            if let Some(status) = &self.save_status {
                ui.label(status);
            }
        } else {
            ui.label("Selecione um arquivo projects.toml para editar.");
        }
    }

    fn load_projects(&mut self, runtime: &ApplicationRuntime) {
        *self.is_loading.lock().unwrap() = true;
        let is_loading_clone = self.is_loading.clone();
        let projects_clone = self.projects.clone();
        let runtime_clone = runtime.clone();

        thread::spawn(move || {
            let res = runtime_clone.list_projects().map_err(|e| e.to_string());
            *projects_clone.lock().unwrap() = Some(res);
            *is_loading_clone.lock().unwrap() = false;
        });
    }
}
