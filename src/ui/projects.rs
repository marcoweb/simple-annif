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
    is_creating: bool,
    form: super::project_form::ProjectForm,
    created_notice: Option<String>,
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
            is_creating: false,
            form: Default::default(),
            created_notice: None,
        }
    }
}

impl ProjectsUi {
    pub fn is_busy(&self) -> bool {
        *self.is_loading.lock().unwrap()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        selected_project: &mut Option<Project>,
        runtime: &ApplicationRuntime,
    ) {
        ui.heading("Projetos Annif");
        ui.add_space(10.0);

        ui.add_enabled_ui(!self.is_busy(), |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(
                        !self.is_editing_toml && !self.is_creating,
                        "Lista de Projetos",
                    )
                    .clicked()
                {
                    self.is_editing_toml = false;
                    self.is_creating = false;
                }
                if ui
                    .selectable_label(self.is_creating, "Novo projeto")
                    .clicked()
                {
                    self.is_creating = true;
                    self.is_editing_toml = false;
                    self.created_notice = None;
                    self.form.refresh(runtime);
                }
                if ui
                    .selectable_label(self.is_editing_toml, "Avançado: editar TOML")
                    .clicked()
                {
                    self.is_editing_toml = true;
                    self.is_creating = false;
                }
            });
        });

        ui.separator();

        if self.is_creating {
            if let Some(project) = self.form.ui(ui, runtime) {
                self.created_notice = Some(format!(
                    "Projeto {} criado e selecionado. {}",
                    project.name,
                    if project.backend == "ensemble" {
                        "Use Suggest depois de treinar os projetos fonte."
                    } else {
                        "Próximo passo: carregue o vocabulário na aba Vocabulários e depois treine o projeto."
                    }
                ));
                *selected_project = Some(project);
                self.is_creating = false;
                self.load_projects(runtime);
            }
        } else if self.is_editing_toml {
            self.ui_edit_toml(ui, runtime);
            if self.projects.lock().unwrap().is_none() {
                *selected_project = None;
            }
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
        if let Some(notice) = &self.created_notice {
            ui.colored_label(egui::Color32::from_rgb(21, 105, 46), notice);
            ui.add_space(8.0);
        }

        if ui
            .add_enabled(!is_loading, egui::Button::new("Atualizar Lista via Annif"))
            .clicked()
        {
            self.load_projects(runtime);
        }

        ui.add_space(10.0);

        if is_loading {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Carregando projetos...");
            });
            return;
        }

        let projects_res = self.projects.lock().unwrap().clone();

        match projects_res {
            Some(Ok(projects)) => {
                if selected_project
                    .as_ref()
                    .is_some_and(|selected| !projects.contains(selected))
                {
                    *selected_project = None;
                }
                if projects.is_empty() {
                    ui.label("Nenhum projeto encontrado.");
                    ui.label("Use Novo projeto para começar, sem precisar escrever TOML.");
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
                ui.colored_label(ui.visuals().error_fg_color, format!("Erro: {}", err));
            }
            None => {
                ui.label("Clique em 'Atualizar Lista' para buscar os projetos.");
            }
        }
    }

    fn ui_edit_toml(&mut self, ui: &mut egui::Ui, runtime: &ApplicationRuntime) {
        ui.label("Salve o TOML em projects/ do workspace ativo para que o Annif o encontre.");
        if self.projects_toml_path.is_empty()
            && let ApplicationRuntime::Embedded(embedded) = runtime
        {
            self.projects_toml_path = embedded
                .workspace()
                .join("projects")
                .join("projects.toml")
                .display()
                .to_string();
            if let Ok(text) = std::fs::read_to_string(&self.projects_toml_path) {
                self.projects_toml_content = text;
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Carregar arquivo indicado").clicked() {
                match std::fs::read_to_string(&self.projects_toml_path) {
                    Ok(text) => {
                        self.projects_toml_content = text;
                        self.save_status = None;
                    }
                    Err(error) => self.save_status = Some(format!("Erro ao ler: {error}")),
                }
            }
            if ui
                .add_enabled(
                    self.projects_toml_content.is_empty(),
                    egui::Button::new("Inserir exemplo TF-IDF / MLLM / Ensemble"),
                )
                .clicked()
            {
                self.projects_toml_content = crate::services::projects::EXAMPLE.into();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Arquivo projects.toml:");
            super::text_input(ui, &mut self.projects_toml_path);
            if ui.button("Procurar...").clicked()
                && let Some(path) = FileDialog::new().add_filter("TOML", &["toml"]).pick_file()
            {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    self.projects_toml_path = path.display().to_string();
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

            let mut save_requested = ui.button("Salvar Alterações").clicked();
            if ui.button("Salvar como...").clicked()
                && let Some(path) = FileDialog::new()
                    .add_filter("TOML", &["toml"])
                    .set_file_name("projects.toml")
                    .save_file()
            {
                self.projects_toml_path = path.display().to_string();
                save_requested = true;
            }
            if save_requested {
                match crate::services::projects::save_for_runtime(
                    runtime,
                    std::path::Path::new(&self.projects_toml_path),
                    &self.projects_toml_content,
                ) {
                    Ok(_) => {
                        self.save_status =
                            Some("Salvo com sucesso! Atualize a lista de projetos.".to_string());
                        *self.projects.lock().unwrap() = None;
                    }
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
            let res = runtime_clone.list_projects().map_err(|e| format!("{e:#}"));
            *projects_clone.lock().unwrap() = Some(res);
            *is_loading_clone.lock().unwrap() = false;
        });
    }
}
