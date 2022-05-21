use crate::config::CustomPlayerEntry;
use eframe::egui::Context;

use eframe::egui::{
    Button, CentralPanel, DragValue, Event, ScrollArea, TextEdit, TextStyle, TopBottomPanel, Window,
};

use super::App;

pub fn update(app: &mut App, ctx: &Context) {
    TopBottomPanel::top("top_panel").show(ctx, |ui| {
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
                app.custom_players_window_show ^= true;
            }
        });
    });
    CentralPanel::default().show(ctx, |ui| {
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
            for ev in &ctx.input().raw.events {
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
    });
    custom_players_window_ui(app, ctx);
}

fn custom_players_window_ui(app: &mut App, ctx: &Context) {
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
