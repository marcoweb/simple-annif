use crate::annif::Project;
use crate::services::runtime::ApplicationRuntime;
use crate::ui::diagnostics::DiagnosticsUi;
use crate::ui::processing::ProcessingUi;
use crate::ui::projects::ProjectsUi;
use crate::ui::training::TrainingUi;
use crate::ui::vocabularies::VocabulariesUi;
use eframe::egui;

pub struct AnnifManagerApp {
    runtime: ApplicationRuntime,
    selected_tab: Tab,
    selected_project: Option<Project>,
    projects_ui: ProjectsUi,
    vocabularies_ui: VocabulariesUi,
    training_ui: TrainingUi,
    processing_ui: ProcessingUi,
    diagnostics_ui: DiagnosticsUi,
}

impl Default for AnnifManagerApp {
    fn default() -> Self {
        Self {
            runtime: ApplicationRuntime::detect(),
            selected_tab: Tab::default(),
            selected_project: None,
            projects_ui: ProjectsUi::default(),
            vocabularies_ui: VocabulariesUi::default(),
            training_ui: TrainingUi::default(),
            processing_ui: ProcessingUi::default(),
            diagnostics_ui: DiagnosticsUi::default(),
        }
    }
}

#[derive(Default, PartialEq)]
enum Tab {
    Projects,
    Vocabularies,
    Training,
    Processing,
    #[default]
    Diagnostics,
}

impl eframe::App for AnnifManagerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

impl AnnifManagerApp {
    fn show(&mut self, ui: &mut egui::Ui) {
        let busy = self.projects_ui.is_busy()
            || self.training_ui.is_busy()
            || self.vocabularies_ui.is_busy()
            || self.processing_ui.is_busy();
        egui::Panel::top("top_panel").show(ui, |ui| {
            egui::menu::MenuBar::new().ui(ui, |ui| {
                ui.selectable_value(&mut self.selected_tab, Tab::Projects, "Projetos");
                ui.selectable_value(&mut self.selected_tab, Tab::Vocabularies, "Vocabulários");
                ui.selectable_value(&mut self.selected_tab, Tab::Training, "Treinamento");
                ui.selectable_value(&mut self.selected_tab, Tab::Processing, "Suggest");
                ui.selectable_value(&mut self.selected_tab, Tab::Diagnostics, "Diagnóstico");
            });
        });

        egui::CentralPanel::default().show(ui, |ui| match self.selected_tab {
            Tab::Projects => {
                ui.add_enabled_ui(!busy || self.projects_ui.is_busy(), |ui| {
                    self.projects_ui
                        .ui(ui, &mut self.selected_project, &self.runtime);
                });
            }
            Tab::Vocabularies => {
                ui.add_enabled_ui(!busy || self.vocabularies_ui.is_busy(), |ui| {
                    self.vocabularies_ui.ui(ui, &self.runtime);
                });
            }
            Tab::Training => {
                ui.add_enabled_ui(!busy || self.training_ui.is_busy(), |ui| {
                    self.training_ui
                        .ui(ui, &self.selected_project, &self.runtime);
                });
            }
            Tab::Processing => {
                ui.add_enabled_ui(!busy || self.processing_ui.is_busy(), |ui| {
                    self.processing_ui
                        .ui(ui, &self.selected_project, &self.runtime);
                });
            }
            Tab::Diagnostics => {
                let before = self.runtime.context_key();
                ui.add_enabled_ui(!busy, |ui| {
                    self.diagnostics_ui.ui(ui, &mut self.runtime);
                });
                if before != self.runtime.context_key() {
                    self.selected_project = None;
                    self.projects_ui = ProjectsUi::default();
                    self.training_ui = TrainingUi::default();
                    self.vocabularies_ui = VocabulariesUi::default();
                    self.processing_ui = ProcessingUi::default();
                }
                if busy {
                    ui.label("Aguarde a operação terminar antes de alterar o ambiente.");
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_tab_paints_a_light_content_background() {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::ThemePreference::Light);
        let mut app = AnnifManagerApp {
            runtime: ApplicationRuntime::Unavailable("Runtime de teste".into()),
            selected_project: Some(Project {
                project_id: "teste".into(),
                name: "Projeto de teste".into(),
                backend: "tfidf".into(),
            }),
            ..Default::default()
        };
        for tab in [
            Tab::Projects,
            Tab::Vocabularies,
            Tab::Training,
            Tab::Processing,
            Tab::Diagnostics,
        ] {
            app.selected_tab = tab;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ui| app.show(ui),
            );
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::epaint::Shape::Rect(rect) if rect.rect.area() > 200_000.0 && rect.fill.r() > 200)),
                "O fundo da área de conteúdo deve ser claro em todas as abas");
            output.textures_delta.clear();
        }
    }
}
