use {
    crate::{
        app::Core,
        config::{Command, CustomPlayerEntry, Predicate, PredicateKind},
    },
    egui_sfml::egui::{self, Color32, ComboBox, Context, RichText, ScrollArea, Ui, Window},
};

#[derive(Default)]
pub struct CustomDemuxersWindow {
    pub open: bool,
    edit_buffer: String,
    edit_target: Option<EditTarget>,
    error_label: String,
    selected_idx: usize,
    tab: CustomDemuxTab,
    selected_pred_idx: usize,
}

#[derive(PartialEq, Default)]
enum CustomDemuxTab {
    #[default]
    Commands,
    Predicates,
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

impl CustomDemuxersWindow {
    pub(super) fn update(&mut self, core: &mut Core, ctx: &Context) {
        let mut open = self.open;
        Window::new("Custom demuxers")
            .open(&mut open)
            .show(ctx, |ui| self.window_ui(core, ui));
        self.open = open;
    }
    fn window_ui(&mut self, core: &mut Core, ui: &mut Ui) {
        let mut idx = 0;
        enum Op {
            None,
            Swap(usize, usize),
            Clone(usize),
        }
        let mut op = Op::None;
        ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            let len = core.cfg.custom_players.len();
            core.cfg.custom_players.retain_mut(|custom_player| {
                let mut retain = true;
                ui.horizontal(|ui| {
                    let label = if custom_player.name.is_empty() {
                        "<unnamed demuxer>"
                    } else {
                        &custom_player.name
                    };
                    if ui
                        .selectable_label(self.selected_idx == idx, label)
                        .clicked()
                    {
                        self.selected_idx = idx;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🗑").on_hover_text("Delete").clicked() {
                            retain = false;
                        }
                        if ui.button("⏶").on_hover_text("Higher prio").clicked() && idx > 0 {
                            op = Op::Swap(idx, idx - 1);
                        }
                        if ui.button("⏷").on_hover_text("Lower prio").clicked() && idx < len - 1 {
                            op = Op::Swap(idx, idx + 1);
                        }
                        if ui.button("🗐").on_hover_text("Clone").clicked() {
                            op = Op::Clone(idx);
                        }
                    });
                    idx += 1;
                });
                retain
            });
        });
        match op {
            Op::None => {}
            Op::Swap(a, b) => core.cfg.custom_players.swap(a, b),
            Op::Clone(idx) => core
                .cfg
                .custom_players
                .insert(idx, core.cfg.custom_players[idx].clone()),
        }
        if ui.button("➕ Add").clicked() {
            core.cfg.custom_players.push(CustomPlayerEntry::default());
        }
        ui.separator();
        if let Some(custom_player) = core.cfg.custom_players.get_mut(self.selected_idx) {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut custom_player.name);
            });
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.tab == CustomDemuxTab::Commands, "Commands")
                    .clicked()
                {
                    self.tab = CustomDemuxTab::Commands;
                }
                if ui
                    .selectable_label(self.tab == CustomDemuxTab::Predicates, "Predicates")
                    .clicked()
                {
                    self.tab = CustomDemuxTab::Predicates;
                }
            });
            ui.separator();
            match self.tab {
                CustomDemuxTab::Commands => {
                    ui.label("Demuxer command");
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
                                    .map(String::from)
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
                }
                CustomDemuxTab::Predicates => {
                    ui.horizontal(|ui| {
                        for i in 0..custom_player.predicates.len() {
                            if ui
                                .selectable_label(i == self.selected_pred_idx, (i + 1).to_string())
                                .clicked()
                            {
                                self.selected_pred_idx = i;
                            }
                        }
                        if ui.button("➕").on_hover_text("Add predicate").clicked() {
                            custom_player
                                .predicates
                                .push(Predicate::HasExt(String::new()));
                        }
                    });
                    if let Some(pred) = custom_player.predicates.get_mut(self.selected_pred_idx) {
                        ComboBox::new(idx, "Kind")
                            .selected_text(PredicateKind::from(&*pred).label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    pred,
                                    Predicate::BeginsWith(String::new()),
                                    PredicateKind::BeginsWith.label(),
                                );
                                ui.selectable_value(
                                    pred,
                                    Predicate::HasExt(String::new()),
                                    PredicateKind::HasExt.label(),
                                );
                            });
                        match pred {
                            Predicate::BeginsWith(frag) => ui.text_edit_singleline(frag),
                            Predicate::HasExt(ext) => ui.text_edit_singleline(ext),
                        };
                    }
                }
            }
        }
    }
}
