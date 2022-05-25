use egui_sfml::egui::{Color32, ComboBox, Context, RichText, Ui, Window};

use crate::{
    app::Core,
    config::{Command, CustomPlayerEntry, Predicate, PredicateKind},
};

#[derive(Default)]
pub struct CustomPlayersWindow {
    pub open: bool,
    edit_buffer: String,
    edit_target: Option<EditTarget>,
    error_label: String,
}

struct EditTarget {
    index: usize,
    which: EditTargetWhich,
}

enum EditTargetWhich {
    Command,
    MpvArgs,
}

impl PredicateKind {
    fn label(&self) -> &str {
        match self {
            PredicateKind::BeginsWith => "Begins with...",
            PredicateKind::HasExt => "Has extension...",
        }
    }
}

impl CustomPlayersWindow {
    pub(super) fn update(&mut self, app: &mut Core, ctx: &Context) {
        let mut open = self.open;
        Window::new("Custom demuxers")
            .open(&mut open)
            .show(ctx, |ui| self.window_ui(app, ui));
        self.open = open;
    }
    fn window_ui(&mut self, app: &mut Core, ui: &mut Ui) {
        let mut idx = 0;
        app.cfg.custom_players.retain_mut(|custom_player| {
            let mut retain = false;
            ui.group(|ui| {
                ComboBox::new(idx, "Predicate")
                    .selected_text(PredicateKind::from(&custom_player.predicate).label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut custom_player.predicate,
                            Predicate::BeginsWith(String::new()),
                            PredicateKind::BeginsWith.label(),
                        );
                        ui.selectable_value(
                            &mut custom_player.predicate,
                            Predicate::HasExt(String::new()),
                            PredicateKind::HasExt.label(),
                        );
                    });
                match &mut custom_player.predicate {
                    Predicate::BeginsWith(frag) => ui.text_edit_singleline(frag),
                    Predicate::HasExt(ext) => ui.text_edit_singleline(ext),
                };
                ui.label("Command");
                match self.edit_target {
                    Some(EditTarget {
                        index,
                        which: EditTargetWhich::Command,
                    }) if idx == index => {
                        if ui.text_edit_singleline(&mut self.edit_buffer).lost_focus() {
                            match Command::from_str(&self.edit_buffer) {
                                Ok(cmd) => {
                                    custom_player.reader_cmd = cmd;
                                    self.error_label.clear();
                                }
                                Err(e) => self.error_label = e.to_string(),
                            }
                            self.edit_buffer.clear();
                            self.edit_target = None;
                        }
                    }
                    _ => {
                        if ui
                            .text_edit_singleline(
                                &mut custom_player.reader_cmd.to_string().unwrap(),
                            )
                            .gained_focus()
                        {
                            self.edit_buffer = custom_player.reader_cmd.to_string().unwrap();
                            self.edit_target = Some(EditTarget {
                                index: idx,
                                which: EditTargetWhich::Command,
                            });
                        }
                    }
                };
                if !self.error_label.is_empty() {
                    ui.label(RichText::new(&self.error_label).color(Color32::RED));
                }
                ui.label("Example: my-cmd --input {}");
                ui.label("extra mpv args");
                match self.edit_target {
                    Some(EditTarget {
                        index,
                        which: EditTargetWhich::MpvArgs,
                    }) if idx == index => {
                        if ui.text_edit_singleline(&mut self.edit_buffer).lost_focus() {
                            custom_player.extra_mpv_args = self
                                .edit_buffer
                                .split_whitespace()
                                .map(|s| s.to_string())
                                .collect();
                            self.edit_buffer.clear();
                            self.edit_target = None;
                        }
                    }
                    _ => {
                        if ui
                            .text_edit_singleline(&mut custom_player.extra_mpv_args.join(" "))
                            .gained_focus()
                        {
                            self.edit_buffer = custom_player.extra_mpv_args.join(" ");
                            self.edit_target = Some(EditTarget {
                                index: idx,
                                which: EditTargetWhich::MpvArgs,
                            });
                        }
                    }
                }
                retain = !ui.button("Delete demuxer").clicked();
            });
            idx += 1;
            retain
        });
        if ui.button("add new demuxer").clicked() {
            app.cfg.custom_players.push(CustomPlayerEntry::default());
        }
    }
}
