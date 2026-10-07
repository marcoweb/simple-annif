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
        egui::Panel::top("top_panel").show(ui, |ui| {
            egui::menu::MenuBar::new().ui(ui, |ui| {
                ui.selectable_value(&mut self.selected_tab, Tab::Projects, "Projetos");
                ui.selectable_value(&mut self.selected_tab, Tab::Vocabularies, "Vocabulários");
                ui.selectable_value(&mut self.selected_tab, Tab::Training, "Treinamento");
                ui.selectable_value(&mut self.selected_tab, Tab::Processing, "Suggest");
                ui.selectable_value(&mut self.selected_tab, Tab::Diagnostics, "Diagnóstico");
            });
        });

        match self.selected_tab {
            Tab::Projects => {
                self.projects_ui
                    .ui(ui, &mut self.selected_project, &self.runtime);
            }
            Tab::Vocabularies => {
                self.vocabularies_ui.ui(ui, &self.runtime);
            }
            Tab::Training => {
                self.training_ui
                    .ui(ui, &self.selected_project, &self.runtime);
            }
            Tab::Processing => {
                self.processing_ui
                    .ui(ui, &self.selected_project, &self.runtime);
            }
            Tab::Diagnostics => {
                self.diagnostics_ui.ui(ui, &mut self.runtime);
            }
        }
    }
}
