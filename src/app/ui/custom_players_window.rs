use eframe::egui::{Context, Window};

use crate::{app::App, config::CustomPlayerEntry};

pub fn update(app: &mut App, ctx: &Context) {
    Window::new("Custom players")
        .open(&mut app.custom_players_window_show)
        .show(ctx, |ui| {
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
        });
}
