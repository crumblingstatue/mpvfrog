mod custom_players_window;

use eframe::egui::{self, ComboBox, Context};

use eframe::egui::{Button, CentralPanel, ScrollArea, TextEdit, TextStyle, TopBottomPanel};

use self::custom_players_window::CustomPlayersWindow;

use super::{AppState, PlaylistBehavior};

#[derive(Default)]
struct Windows {
    custom_players: CustomPlayersWindow,
}

impl Windows {
    fn update(&mut self, app: &mut AppState, ctx: &Context) {
        self.custom_players.update(app, ctx);
    }
}

#[derive(Default)]
pub struct Ui {
    windows: Windows,
}

impl Ui {
    pub(super) fn update(&mut self, app: &mut AppState, ctx: &Context) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| self.top_panel_ui(app, ui));
        CentralPanel::default().show(ctx, |ui| self.central_panel_ui(app, ui));
        self.windows.update(app, ctx);
    }
    fn top_panel_ui(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.group(|ui| {
                if ui.button("Music folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        app.cfg.music_folder = Some(path);
                        app.read_songs();
                    }
                }
                match &app.cfg.music_folder {
                    Some(folder) => {
                        ui.label(&folder.display().to_string());
                    }
                    None => {
                        ui.label("<none>");
                    }
                }
            });
            if ui.button("Custom players...").clicked() {
                self.windows.custom_players.open ^= true;
            }
        });
    }
    fn central_panel_ui(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        ScrollArea::vertical()
            .max_height(200.0)
            .id_source("song_scroll")
            .show(ui, |ui| {
                for (i, path) in app.playlist.iter().enumerate() {
                    if ui
                        .selectable_label(app.selected_song == i, path.display().to_string())
                        .clicked()
                    {
                        app.selected_song = i;
                        app.play_selected_song();
                        break;
                    }
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.group(|ui| {
                if ui.add(Button::new("⏪")).clicked() {
                    app.play_prev();
                }
                let active = app.mpv_handler.active();
                let icon = if active && !app.mpv_handler.paused() {
                    "⏸"
                } else {
                    "▶"
                };
                if ui.add(Button::new(icon)).clicked() {
                    if active {
                        app.mpv_handler.toggle_pause();
                    } else {
                        app.play_selected_song();
                    }
                }
                if ui.add_enabled(active, Button::new("⏹")).clicked() {
                    app.stop_music();
                }
                if ui.add(Button::new("⏩")).clicked() {
                    app.play_next();
                }
            });
            ui.group(|ui| {
                ui.label("🔈");
                ui.label(app.cfg.volume.to_string());
            });
            ui.group(|ui| {
                ui.label("⏩");
                ui.label(format!("{:.2}", app.cfg.speed));
            });
            ui.group(|ui| {
                ComboBox::new("playlist_behavior_cb", "▶")
                    .selected_text(app.playlist_behavior.label())
                    .show_ui(ui, |ui| {
                        use self::PlaylistBehavior::*;
                        ui.selectable_value(&mut app.playlist_behavior, Stop, Stop.label());
                        ui.selectable_value(&mut app.playlist_behavior, Continue, Continue.label());
                        ui.selectable_value(
                            &mut app.playlist_behavior,
                            RepeatOne,
                            RepeatOne.label(),
                        );
                        ui.selectable_value(
                            &mut app.playlist_behavior,
                            RepeatPlaylist,
                            RepeatPlaylist.label(),
                        );
                    })
            });
        });
        ui.separator();
        ScrollArea::vertical()
            .id_source("out_scroll")
            .stick_to_bottom()
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut app.mpv_handler.mpv_output().as_str())
                        .desired_width(620.0)
                        .font(TextStyle::Monospace),
                );
            });
    }
}

impl PlaylistBehavior {
    fn label(&self) -> &'static str {
        match self {
            PlaylistBehavior::Stop => "Stop",
            PlaylistBehavior::Continue => "Continue",
            PlaylistBehavior::RepeatOne => "Repeat one",
            PlaylistBehavior::RepeatPlaylist => "Repeat playlist",
        }
    }
}
