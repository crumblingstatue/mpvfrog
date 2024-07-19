mod custom_demuxers_window;

use {
    self::custom_demuxers_window::CustomDemuxersWindow,
    super::{Core, PlaylistBehavior, LOG},
    crate::{bool_ext::BoolExt, MODAL},
    egui_sfml::egui::{
        self, Align, Button, CentralPanel, ComboBox, Context, ScrollArea, TextEdit, TextStyle,
        TopBottomPanel,
    },
    std::fmt,
};

#[derive(Default)]
struct Windows {
    custom_demuxers: CustomDemuxersWindow,
}

impl Windows {
    fn update(&mut self, app: &mut Core, ctx: &Context) {
        self.custom_demuxers.update(app, ctx);
    }
}

#[derive(Default)]
pub struct Ui {
    windows: Windows,
    filter_string: String,
    /// This is set to true when filter string has been changed.
    ///
    /// When this happens, we'll try to scroll to the selected song if we can
    filter_changed: bool,
    output_source: OutputSource,
    file_dialog: egui_file_dialog::FileDialog,
}

#[derive(Default, PartialEq, Eq)]
enum OutputSource {
    #[default]
    Mpv,
    Demuxer,
    Log,
}

impl Ui {
    pub(super) fn update(&mut self, app: &mut Core, ctx: &Context) {
        if let Some(modal) = &mut *MODAL.lock().unwrap() {
            modal.show_dialog();
        }
        TopBottomPanel::top("top_panel").show(ctx, |ui| self.top_panel_ui(app, ui));
        CentralPanel::default().show(ctx, |ui| self.central_panel_ui(app, ui));
        self.windows.update(app, ctx);
    }
    fn top_panel_ui(&mut self, app: &mut Core, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.group(|ui| {
                if ui.button("Music folder").clicked() {
                    self.file_dialog.select_directory();
                }
                if let Some(path) = self.file_dialog.take_selected() {
                    app.cfg.music_folder = Some(path);
                    app.read_songs();
                }
                self.file_dialog.update(ui.ctx());
                match &app.cfg.music_folder {
                    Some(folder) => {
                        ui.label(folder.display().to_string());
                    }
                    None => {
                        ui.label("<none>");
                    }
                }
            });
            if ui.button("Custom demuxers...").clicked() {
                self.windows.custom_demuxers.open ^= true;
            }
            ui.label("🔎");
            if ui
                .add(TextEdit::singleline(&mut self.filter_string).hint_text("Filter"))
                .changed()
            {
                self.filter_changed = true;
            }
            self.filter_string = self.filter_string.to_ascii_lowercase();
        });
    }
    fn central_panel_ui(&mut self, app: &mut Core, ui: &mut egui::Ui) {
        ScrollArea::vertical()
            .max_height(200.0)
            .auto_shrink([false; 2])
            .id_source("song_scroll")
            .show(ui, |ui| {
                for (i, path) in app.playlist.iter().enumerate() {
                    if !self.filter_string.is_empty() {
                        match path.to_str() {
                            Some(path_str) => {
                                if !path_str.to_ascii_lowercase().contains(&self.filter_string) {
                                    continue;
                                }
                            }
                            None => continue,
                        }
                    }
                    let re =
                        ui.selectable_label(app.selected_song == i, path.display().to_string());
                    if app.selected_song == i
                        && (self.filter_changed.take() || app.song_change.take())
                    {
                        re.scroll_to_me(Some(Align::Center));
                    }
                    if re.clicked() {
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
                match app.mpv_handler.volume() {
                    Some(mut vol) => {
                        ui.style_mut().spacing.slider_width = 160.0;
                        let re = ui.add(egui::Slider::new(&mut vol, 0..=150));
                        if re.changed() {
                            app.mpv_handler.set_volume(vol);
                        }
                    }
                    None => {
                        ui.add(egui::Slider::new(&mut app.cfg.volume, 0..=150));
                    }
                }
            });
            ui.group(|ui| {
                ui.label("⏩");
                match app.mpv_handler.speed() {
                    Some(mut speed) => {
                        ui.style_mut().spacing.slider_width = 160.0;
                        let re = ui.add(egui::Slider::new(&mut speed, 0.3..=2.0));
                        if re.changed() {
                            app.mpv_handler.set_speed(speed);
                        }
                    }
                    None => {
                        ui.add(egui::Slider::new(&mut app.cfg.speed, 0.3..=2.0));
                    }
                }
            });
            if ui.checkbox(&mut app.cfg.video, "video").clicked() {
                app.set_video(app.cfg.video);
            }
        });
        ui.horizontal(|ui| {
            if let Some(mut info) = app.mpv_handler.time_info() {
                ui.style_mut().spacing.slider_width = 420.0;
                ui.label(format!(
                    "{}/{}",
                    FfmpegTimeFmt(info.pos),
                    FfmpegTimeFmt(info.duration)
                ));
                let re = ui.add(
                    egui::Slider::new(&mut info.pos, 0.0..=info.duration)
                        .show_value(false)
                        .trailing_fill(true),
                );
                if re.drag_stopped() {
                    app.seek(info.pos);
                }
            }
            ui.group(|ui| {
                ui.style_mut().spacing.slider_width = 100.0;
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
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.output_source, OutputSource::Mpv, "Mpv");
            ui.selectable_value(&mut self.output_source, OutputSource::Demuxer, "Demuxer");
            ui.selectable_value(&mut self.output_source, OutputSource::Log, "Log");
        });
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .id_source("out_scroll")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let out = match self.output_source {
                    OutputSource::Mpv => app.mpv_handler.mpv_output(),
                    OutputSource::Demuxer => app.mpv_handler.demux_term.contents_to_string(),
                    OutputSource::Log => LOG.lock().unwrap().clone(),
                };
                ui.add(
                    TextEdit::multiline(&mut out.as_str())
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

pub struct FfmpegTimeFmt(pub f64);

impl fmt::Display for FfmpegTimeFmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0;
        let hh = secs / 3600.0;
        let mm = hh.fract() * 60.0;
        let ss = mm.fract() * 60.0;
        write!(
            f,
            "{:02.0}:{:02.0}:{:02.0}.{:03}",
            hh.floor(),
            mm.floor(),
            ss.floor(),
            (ss.fract() * 1000.0).round() as u64
        )
    }
}

#[test]
fn test_time_fmt() {
    assert_eq!(&FfmpegTimeFmt(0.0).to_string()[..], "00:00:00.000");
    assert_eq!(&FfmpegTimeFmt(24.56).to_string()[..], "00:00:24.560");
    assert_eq!(&FfmpegTimeFmt(119.885).to_string()[..], "00:01:59.885");
    assert_eq!(&FfmpegTimeFmt(52349.345).to_string()[..], "14:32:29.345");
}
