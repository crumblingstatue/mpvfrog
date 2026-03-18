use {
    crate::{app::core::Core, ipc::property::LavfiComplex, logln},
    egui_sf2g::egui,
};

#[derive(Default)]
pub struct MpvConsoleWindow {
    pub open: bool,
    cmd_buf: String,
}

const HELP: &str = "\
Help:
    lavfi <str> - Set lavfi-complex filter
";

impl MpvConsoleWindow {
    pub(crate) fn update(&mut self, core: &mut Core, ctx: &egui::Context) {
        egui::Window::new("🖳 Mpv console")
            .open(&mut self.open)
            .show(ctx, |ui| {
                ui.label(HELP);
                ui.text_edit_singleline(&mut self.cmd_buf);
                if ctx.input(|inp| inp.key_pressed(egui::Key::Enter))
                    && let Some((cmd, args)) = self.cmd_buf.split_once(' ')
                {
                    match cmd {
                        "lavfi" => {
                            core.mpv_handler
                                .ipc(|ipc| ipc.set_property::<LavfiComplex>(args.into()));
                        }
                        _ => logln!("Unknown command: {cmd}"),
                    }
                };
                ui.code(crate::app::LOG.lock().unwrap().as_str());
            });
    }
}
