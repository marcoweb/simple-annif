pub mod diagnostics;
pub mod processing;
pub mod project_form;
pub mod projects;
pub mod training;
pub mod vocabularies;

/// Text fields also accept replacement through Windows UI Automation.
pub fn text_input(ui: &mut eframe::egui::Ui, value: &mut String) -> eframe::egui::Response {
    use eframe::egui::accesskit::{Action, ActionData};
    let mut response = ui.text_edit_singleline(value);
    if response.enabled() {
        ui.input(|input| {
            for request in input.accesskit_action_requests(response.id, Action::SetValue) {
                if let Some(ActionData::Value(replacement)) = &request.data {
                    *value = replacement.to_string();
                    response.mark_changed();
                }
            }
        });
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.add_action(Action::SetValue));
    }
    if response.changed() {
        ui.ctx().request_repaint();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{
        self,
        accesskit::{Action, ActionData, ActionRequest},
    };

    #[test]
    fn accessible_text_replacement_preserves_unicode_and_respects_disabled_fields() {
        let ctx = egui::Context::default();
        let mut value = "original".to_owned();
        let mut id = egui::Id::NULL;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            id = text_input(ui, &mut value).id;
        });
        output.textures_delta.clear();
        for enabled in [false, true] {
            let input = egui::RawInput {
                events: vec![egui::Event::AccessKitActionRequest(ActionRequest {
                    target_tree: egui::accesskit::TreeId::ROOT,
                    target_node: id.accesskit_id(),
                    action: Action::SetValue,
                    data: Some(ActionData::Value("C:\\ação com espaços\\nalt.rdf".into())),
                })],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                if !enabled {
                    ui.disable();
                }
                text_input(ui, &mut value);
            });
            output.textures_delta.clear();
            assert_eq!(
                value,
                if enabled {
                    "C:\\ação com espaços\\nalt.rdf"
                } else {
                    "original"
                }
            );
        }
    }
}
