use eframe::egui::{Context, Ui, Window};

use crate::{app::Core, config::CustomPlayerEntry};

#[derive(Default)]
pub struct CustomPlayersWindow {
    pub open: bool,
}

impl CustomPlayersWindow {
    pub(super) fn update(&mut self, app: &mut Core, ctx: &Context) {
        Window::new("Custom players")
            .open(&mut self.open)
            .show(ctx, |ui| window_ui(app, ui));
    }
}

fn window_ui(app: &mut Core, ui: &mut Ui) {
    for en in &mut app.cfg.custom_players {
        ui.group(|ui| {
            ui.label("extension");
            ui.text_edit_singleline(&mut en.ext);
            ui.label("command");
            ui.text_edit_singleline(&mut en.cmd);
            ui.label("args");
            for arg in &mut en.args {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(arg);
                    if ui.button("...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            *arg = path.to_string_lossy().into_owned();
                        }
                    }
                });
            }
            if ui.button("+").clicked() {
                en.args.push(String::new());
            }
        });
    }
    if ui.button("add new custom player").clicked() {
        app.cfg.custom_players.push(CustomPlayerEntry::default());
    }
}
