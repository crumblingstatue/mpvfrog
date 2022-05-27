use ksni::{menu::StandardItem, Tray, TrayService};

#[derive(Default)]
pub struct AppTray {
    pub should_toggle_window: bool,
    pub should_quit: bool,
    pub paused: bool,
    pub should_pause_resume: bool,
    pub more_info_label: String,
}

impl Tray for AppTray {
    fn activate(&mut self, _x: i32, _y: i32) {
        self.should_toggle_window = true;
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: if self.paused { "▶" } else { " ⏸" }.into(),
                activate: Box::new(|this: &mut Self| {
                    this.should_pause_resume = true;
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    this.should_quit = true;
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
        let title = if !self.more_info_label.is_empty() {
            format!("mpv-egui\n{}", self.more_info_label)
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
