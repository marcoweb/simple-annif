use crate::services::{
    project_creation::{self, ConfiguredProject, Draft},
    runtime::ApplicationRuntime,
};
use eframe::egui;

#[derive(Default)]
pub struct ProjectForm {
    draft: Draft,
    catalog: Vec<ConfiguredProject>,
    error: Option<String>,
    catalog_error: Option<String>,
}

impl ProjectForm {
    pub fn refresh(&mut self, runtime: &ApplicationRuntime) {
        match project_creation::catalog(runtime) {
            Ok(items) => {
                self.catalog = items;
                self.catalog_error = None;
            }
            Err(error) => {
                self.catalog.clear();
                self.catalog_error = Some(format!("{error:#}"));
            }
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        runtime: &ApplicationRuntime,
    ) -> Option<crate::annif::Project> {
        let mut created = None;
        egui::ScrollArea::vertical().id_salt("new-project-form").show(ui, |ui| {
            ui.heading("Novo projeto");
            ui.label("Defina o que deseja classificar. A configuração será salva no workspace automaticamente.");
            ui.add_space(12.0);
            ui.strong("Nome do projeto");
            let old_name = self.draft.name.clone();
            if super::text_input(ui, &mut self.draft.name).changed()
                && (self.draft.id.is_empty() || self.draft.id == project_creation::suggested_id(&old_name)) {
                self.draft.id = project_creation::suggested_id(&self.draft.name);
            }
            ui.small("Exemplo: Agricultura em inglês");
            ui.horizontal_wrapped(|ui| {
                ui.label("Identificador:");
                super::text_input(ui, &mut self.draft.id);
            });
            ui.small("Sugerido a partir do nome. Deve ser único, sem espaços.");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.strong("Idioma dos documentos");
                egui::ComboBox::from_id_salt("new-project-language")
                    .selected_text(match self.draft.language.as_str() { "en" => "Inglês", "es" => "Espanhol", _ => "Português" })
                    .show_ui(ui, |ui| {
                        for (code,label) in [("pt","Português"),("en","Inglês"),("es","Espanhol")] {
                            ui.selectable_value(&mut self.draft.language, code.into(), label);
                        }
                    });
            });
            ui.add_space(12.0);
            ui.strong("Vocabulário de assuntos");
            super::text_input(ui, &mut self.draft.vocab);
            let mut vocabs: Vec<_> = self.catalog.iter().map(|p| p.vocab.clone()).filter(|v| !v.is_empty()).collect();
            vocabs.sort(); vocabs.dedup();
            if !vocabs.is_empty() {
                egui::ComboBox::from_id_salt("existing-project-vocab").selected_text("Usar vocabulário existente…").show_ui(ui, |ui| {
                    for vocab in vocabs { ui.selectable_value(&mut self.draft.vocab, vocab.clone(), vocab); }
                });
            }
            ui.small("Informe um ID, como nalt. Depois, carregue o arquivo na aba Vocabulários. O mesmo ID pode ser usado por vários projetos.");
            ui.add_space(12.0);
            ui.strong("Método de classificação");
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(&mut self.draft.backend, "tfidf".into(), "TF-IDF");
                ui.radio_value(&mut self.draft.backend, "mllm".into(), "MLLM");
                ui.radio_value(&mut self.draft.backend, "ensemble".into(), "Ensemble");
            });
            ui.label(match self.draft.backend.as_str() {
                "mllm" => "MLLM procura termos do vocabulário no texto e aprende com documentos já classificados.",
                "ensemble" => "Ensemble combina as sugestões de projetos existentes. Treine os projetos fonte antes de sugerir descritores.",
                _ => "TF-IDF usa a importância das palavras nos documentos de treinamento. Uma opção simples para começar.",
            });
            if self.draft.backend == "ensemble" {
                ui.add_space(6.0);
                ui.strong("Projetos para combinar");
                self.draft.sources.retain(|id| self.catalog.iter().any(|p| &p.id == id && p.language == self.draft.language
                    && p.vocab == self.draft.vocab.trim() && ["tfidf", "mllm"].contains(&p.backend.as_str())));
                let candidates: Vec<_> = self.catalog.iter().filter(|p| p.language == self.draft.language
                    && p.vocab == self.draft.vocab.trim() && ["tfidf", "mllm"].contains(&p.backend.as_str())).collect();
                for p in &candidates {
                    let mut selected = self.draft.sources.contains(&p.id);
                    if ui.checkbox(&mut selected, format!("{} ({})", p.name, p.id)).changed() {
                        if selected { self.draft.sources.push(p.id.clone()); }
                        else { self.draft.sources.retain(|id| id != &p.id); }
                    }
                }
                if candidates.len() < 2 { ui.label("Crie primeiro dois projetos TF-IDF ou MLLM com o mesmo idioma e vocabulário."); }
                ui.small("Os projetos selecionados terão pesos iguais. Ajustes de pesos ficam no editor avançado.");
            }
            ui.add_space(12.0);
            let validation = project_creation::validate(&self.draft).err();
            if let Some(error) = &validation { ui.label(error.to_string()); }
            if let Some(error) = &self.catalog_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.button("Tentar ler configurações novamente").clicked() { self.refresh(runtime); }
            }
            if ui.add_enabled(validation.is_none() && self.catalog_error.is_none(), egui::Button::new("Criar projeto")).clicked() {
                match project_creation::create(runtime, &self.draft) {
                    Ok(_) => {
                        created = Some(crate::annif::Project { project_id: self.draft.id.clone(), name: self.draft.name.trim().into(), backend: self.draft.backend.clone() });
                        *self = Self::default();
                    }
                    Err(error) => self.error = Some(format!("{error:#}")),
                }
            }
            if let Some(error) = &self.error { ui.colored_label(ui.visuals().error_fg_color, error); }
        });
        created
    }
}
