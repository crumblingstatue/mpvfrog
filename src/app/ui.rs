mod color_theme_window;
mod custom_demuxers_window;

use {
    self::custom_demuxers_window::CustomDemuxersWindow,
    super::{Core, LOG, PlaylistBehavior},
    crate::{MODAL, bool_ext::BoolExt, mpv_handler::ActivePtyInput},
    color_theme_window::ColorThemeWindow,
    egui_colors::{Colorix, tokens::ThemeColor},
    egui_sfml::egui::{
        self, Align, Button, CentralPanel, ComboBox, Context, ScrollArea, TextEdit, TextStyle,
        TopBottomPanel,
    },
    fuzzy_matcher::{FuzzyMatcher as _, skim::SkimMatcherV2},
    std::fmt,
};

#[derive(Default)]
struct Windows {
    custom_demuxers: CustomDemuxersWindow,
    color_theme: ColorThemeWindow,
}

impl Windows {
    fn update(&mut self, core: &mut Core, ctx: &Context, colorix: &mut Option<Colorix>) {
        self.custom_demuxers.update(core, ctx);
        self.color_theme.update(core, ctx, colorix);
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
    colorix: Option<Colorix>,
    filtered_entries: Vec<usize>,
}

#[derive(Default, PartialEq, Eq)]
enum OutputSource {
    #[default]
    Mpv,
    Demuxer,
    Log,
}

impl Ui {
    pub(super) fn update(&mut self, core: &mut Core, ctx: &Context) {
        if let Some(modal) = &mut *MODAL.lock().unwrap() {
            modal.show_dialog();
        }
        TopBottomPanel::top("top_panel").show(ctx, |ui| self.top_panel_ui(core, ui));
        CentralPanel::default().show(ctx, |ui| self.central_panel_ui(core, ui));
        self.windows.update(core, ctx, &mut self.colorix);
    }
    fn top_panel_ui(&mut self, core: &mut Core, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.label(crate::APP_LABEL);
            ui.group(|ui| {
                if ui.button("Music folder").clicked() {
                    self.file_dialog.select_directory();
                }
                if let Some(path) = self.file_dialog.take_selected() {
                    core.cfg.music_folder = Some(path);
                    core.read_songs();
                }
                self.file_dialog.update(ui.ctx());
                match &core.cfg.music_folder {
                    Some(folder) => {
                        ui.label(folder.display().to_string());
                    }
                    None => {
                        ui.label("<none>");
                    }
                }
                if ui.button("🔃").on_hover_text("Refresh (F5)").clicked()
                    || ui.input(|inp| inp.key_pressed(egui::Key::F5))
                {
                    core.read_songs();
                }
            });
            if ui.button("Custom demuxers...").clicked() {
                self.windows.custom_demuxers.open ^= true;
            }
            ui.label("🔎");
            let ctrl_f = ui.input(|inp| inp.key_pressed(egui::Key::F) && inp.modifiers.ctrl);
            let re =
                ui.add(TextEdit::singleline(&mut self.filter_string).hint_text("Filter (ctrl+f)"));
            if re.changed() {
                self.filter_changed = true;
                self.recalc_filt_entries(core);
            }
            if ctrl_f {
                re.request_focus();
            }
            if ui
                .button("💎")
                .on_hover_text("Color theme config")
                .clicked()
            {
                self.windows.color_theme.open ^= true;
            }
        });
    }

    pub(crate) fn recalc_filt_entries(&mut self, core: &Core) {
        let matcher = SkimMatcherV2::default();
        let prepared_filter = self.filter_string.replace(char::is_whitespace, "");
        let mut scored_indices: Vec<(usize, i64)> = core
            .playlist
            .iter()
            .enumerate()
            .filter_map(|(idx, path)| {
                path.to_str().and_then(|path_str| {
                    matcher
                        .fuzzy_match(path_str, &prepared_filter)
                        .map(|score| (idx, score))
                })
            })
            .collect();
        scored_indices.sort_by(|(_, score1), (_, score2)| score1.cmp(score2).reverse());
        self.filtered_entries = scored_indices
            .into_iter()
            .map(|(idx, _score)| idx)
            .collect();
    }

    fn central_panel_ui(&mut self, core: &mut Core, ui: &mut egui::Ui) {
        ScrollArea::vertical()
            .max_height(200.0)
            .auto_shrink([false; 2])
            .id_salt("song_scroll")
            .show(ui, |ui| {
                for &i in &self.filtered_entries {
                    let path = &core.playlist[i];
                    let re =
                        ui.selectable_label(core.selected_song == i, path.display().to_string());
                    let filter_changed = self.filter_changed.take();
                    if filter_changed {
                        ui.scroll_to_rect(egui::Rect::ZERO, Some(Align::TOP));
                    }
                    if core.selected_song == i && (filter_changed || core.song_change.take()) {
                        re.scroll_to_me(Some(Align::Center));
                    }
                    if re.clicked() {
                        core.selected_song = i;
                        core.play_selected_song();
                        break;
                    }
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.group(|ui| {
                if ui.add(Button::new("⏪")).clicked() {
                    core.play_prev();
                }
                let active = core.mpv_handler.active();
                let icon = if active && !core.mpv_handler.paused() {
                    "⏸"
                } else {
                    "▶"
                };
                if ui.add(Button::new(icon)).clicked() {
                    if active {
                        core.mpv_handler.toggle_pause();
                    } else {
                        core.play_selected_song();
                    }
                }
                if ui.add_enabled(active, Button::new("⏹")).clicked() {
                    core.stop_music();
                }
                if ui.add(Button::new("⏩")).clicked() {
                    core.play_next();
                }
            });
            ui.group(|ui| {
                ui.label("🔈");
                match core.mpv_handler.volume() {
                    Some(mut vol) => {
                        ui.style_mut().spacing.slider_width = 160.0;
                        let re = ui.add(egui::Slider::new(&mut vol, 0..=150));
                        if re.changed() {
                            core.mpv_handler.set_volume(vol);
                        }
                    }
                    None => {
                        ui.add(egui::Slider::new(&mut core.cfg.volume, 0..=150));
                    }
                }
            });
            ui.group(|ui| {
                ui.label("⏩");
                match core.mpv_handler.speed() {
                    Some(mut speed) => {
                        ui.style_mut().spacing.slider_width = 160.0;
                        let re = ui.add(egui::Slider::new(&mut speed, 0.3..=2.0));
                        if re.changed() {
                            core.mpv_handler.set_speed(speed);
                        }
                    }
                    None => {
                        ui.add(egui::Slider::new(&mut core.cfg.speed, 0.3..=2.0));
                    }
                }
            });
            if ui.checkbox(&mut core.cfg.video, "video").clicked() {
                core.set_video(core.cfg.video);
            }
        });
        ui.horizontal(|ui| {
            if let Some(mut info) = core.mpv_handler.time_info() {
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
                    core.seek(info.pos);
                }
            }
            ui.group(|ui| {
                ui.style_mut().spacing.slider_width = 100.0;
                ComboBox::new("playlist_behavior_cb", "▶")
                    .selected_text(core.playlist_behavior.label())
                    .show_ui(ui, |ui| {
                        use self::PlaylistBehavior::*;
                        ui.selectable_value(&mut core.playlist_behavior, Stop, Stop.label());
                        ui.selectable_value(
                            &mut core.playlist_behavior,
                            Continue,
                            Continue.label(),
                        );
                        ui.selectable_value(
                            &mut core.playlist_behavior,
                            RepeatOne,
                            RepeatOne.label(),
                        );
                        ui.selectable_value(
                            &mut core.playlist_behavior,
                            RepeatPlaylist,
                            RepeatPlaylist.label(),
                        );
                    })
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .selectable_value(&mut self.output_source, OutputSource::Mpv, "Mpv")
                .clicked()
            {
                core.mpv_handler.active_pty_input = ActivePtyInput::Mpv;
            }
            let mut demux_enabled = true;
            if !core.mpv_handler.demuxer_active() && core.mpv_handler.demux_term.is_empty() {
                demux_enabled = false;
            }
            if ui
                .add_enabled(
                    demux_enabled,
                    egui::SelectableLabel::new(
                        self.output_source == OutputSource::Demuxer,
                        "Demuxer",
                    ),
                )
                .on_disabled_hover_text("No active demuxer")
                .clicked()
            {
                self.output_source = OutputSource::Demuxer;
                core.mpv_handler.active_pty_input = ActivePtyInput::Demuxer;
            };
            ui.selectable_value(&mut self.output_source, OutputSource::Log, "Log");
        });
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .id_salt("out_scroll")
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let out = match self.output_source {
                    OutputSource::Mpv => core.mpv_handler.mpv_output(),
                    OutputSource::Demuxer => core.mpv_handler.demux_term.contents_to_string(),
                    OutputSource::Log => LOG.lock().unwrap().clone(),
                };
                // Weird hack to make PTY interaction work even if the TextEdit was clicked.
                // Normally, the `TextEdit` is interested in keyboard events even in the
                // "immutable" mode, which is not what we want.
                // But unconditionally surrendering focus also deselects, so we first check if
                // nothing is being selected.
                let out = TextEdit::multiline(&mut out.as_str())
                    .desired_width(620.0)
                    .font(TextStyle::Monospace)
                    .show(ui);
                if out
                    .cursor_range
                    .is_none_or(|range| range.primary == range.secondary)
                {
                    out.response.surrender_focus();
                }
            });
    }
    pub fn apply_colorix_theme(&mut self, theme: &Option<[[u8; 3]; 12]>, ctx: &Context) {
        if let Some(theme) = theme {
            self.colorix = Some(Colorix::init(
                ctx,
                std::array::from_fn(|i| ThemeColor::Custom(theme[i])),
            ));
        }
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
