use {crate::app::core::Core, egui_sf2g::egui};

#[derive(Default)]
pub struct MpvConsoleWindow {
    pub open: bool,
    cmd_buf: String,
    backlog: String,
}

impl MpvConsoleWindow {
    pub(crate) fn update(&mut self, core: &mut Core, ctx: &egui::Context) {
        egui::Window::new("🖳 Mpv console")
            .open(&mut self.open)
            .show(ctx, |ui| {
                ui.label(&self.backlog);
                ui.text_edit_singleline(&mut self.cmd_buf);
                if ctx.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                    let mut tokens = self.cmd_buf.split_whitespace();
                    let Some(cmd) = tokens.next() else {
                        return;
                    };
                    let mut ipc_msg = format!("{{\"command\": [\"{cmd}\",");
                    for arg in tokens {
                        ipc_msg.push_str(&format!("{arg},"));
                    }
                    // remove trailing comma
                    ipc_msg.pop();
                    ipc_msg.push_str("]}");
                    self.backlog.push_str(&ipc_msg);
                    self.backlog.push('\n');
                    core.mpv_handler.ipc(|b| b.write_str(&ipc_msg));
                    self.cmd_buf.clear();
                }
            });
    }
}
