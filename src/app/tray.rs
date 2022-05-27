use ksni::{menu::StandardItem, Tray, TrayService};

#[derive(Default)]
pub struct AppTray {
    pub event_flags: EventFlags,
    pub app_state: AppState,
}

#[derive(Default)]
pub struct EventFlags {
    pub activated: bool,
    pub quit_clicked: bool,
    pub pause_resume_clicked: bool,
}

impl EventFlags {
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

#[derive(Default)]
pub struct AppState {
    pub tray_info: String,
    pub paused: bool,
}

impl Tray for AppTray {
    fn activate(&mut self, _x: i32, _y: i32) {
        self.event_flags.activated = true;
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: if self.app_state.paused { "▶" } else { " ⏸" }.into(),
                activate: Box::new(|this: &mut Self| {
                    this.event_flags.pause_resume_clicked = true;
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.event_flags.quit_clicked = true;
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 32,
            height: 32,
            data: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon.argb32")).to_vec(),
        }]
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        let title = if !self.app_state.tray_info.is_empty() {
            format!("mpv-egui\n{}", self.app_state.tray_info)
        } else {
            "mpv-egui".into()
        };
        ksni::ToolTip {
            title,
            ..Default::default()
        }
    }
}

impl AppTray {
    pub fn spawn() -> ksni::Handle<Self> {
        let tray_service = TrayService::new(AppTray::default());
        let tray_handle = tray_service.handle();
        tray_service.spawn();
        tray_handle
    }
}
