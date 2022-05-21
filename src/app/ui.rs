mod custom_players_window;

use eframe::egui::{self, Context};

use eframe::egui::{
    Button, CentralPanel, DragValue, Event, ScrollArea, TextEdit, TextStyle, TopBottomPanel,
};

use self::custom_players_window::CustomPlayersWindow;

use super::AppState;

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
                    app.cfg.music_folder = rfd::FileDialog::new().pick_folder();
                    app.read_songs();
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
                for (i, path) in app.song_paths.iter().enumerate() {
                    if ui
                        .selectable_label(app.selected_song == Some(i), path.display().to_string())
                        .clicked()
                    {
                        app.selected_song = Some(i);
                        app.play_selected_song();
                        break;
                    }
                }
            });
        if app.mpv_handler.active() {
            for ev in &ui.ctx().input().raw.events {
                if let Event::Text(s) = ev {
                    match s.as_str() {
                        " " => app.mpv_handler.toggle_pause(),
                        _ => app.mpv_handler.input(s),
                    }
                }
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.group(|ui| {
                if ui
                    .add_enabled(app.selected_song.is_some(), Button::new("⏪"))
                    .clicked()
                {
                    if let Some(sel) = &mut app.selected_song {
                        *sel = sel.saturating_sub(1);
                        app.play_selected_song();
                    }
                }
                let active = app.mpv_handler.active();
                let icon = if active && !app.mpv_handler.paused() {
                    "⏸"
                } else {
                    "▶"
                };
                if ui
                    .add_enabled(app.selected_song.is_some(), Button::new(icon))
                    .clicked()
                {
                    if active {
                        app.mpv_handler.toggle_pause();
                    } else {
                        app.play_selected_song();
                    }
                }
                if ui.add_enabled(active, Button::new("⏹")).clicked() {
                    app.mpv_handler.stop_music();
                }
                let can_forward = app
                    .selected_song
                    .map_or(false, |sel| sel + 1 < app.song_paths.len());
                if ui.add_enabled(can_forward, Button::new("⏩")).clicked() {
                    if let Some(sel) = &mut app.selected_song {
                        *sel += 1;
                        app.play_selected_song();
                    }
                }
            });
            ui.group(|ui| {
                ui.label("🔈");
                ui.add(DragValue::new(&mut app.cfg.volume));
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
