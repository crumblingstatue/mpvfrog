use {
    crate::util::result_ext::LogErrExt,
    crossbeam_channel::{Receiver, Sender},
    std::sync::Mutex,
    zbus::{blocking::connection, interface, object_server::SignalEmitter},
};

pub struct AppTray {
    pub event_flags: EventFlags,
    pub sender: Sender<AppToTrayMsg>,
    receiver: Receiver<TrayToAppMsg>,
    pub conn: connection::Connection,
}

impl AppTray {
    pub fn update(&mut self) {
        if let Ok(msg) = self.receiver.try_recv() {
            match msg {
                TrayToAppMsg::ShowCtxMenu { x, y } => self.event_flags.ctx_menu = Some((x, y)),
                TrayToAppMsg::Activate => self.event_flags.activated = true,
            }
        }
    }
}

pub enum TrayToAppMsg {
    ShowCtxMenu { x: i32, y: i32 },
    Activate,
}

#[derive(Debug)]
pub enum AppToTrayMsg {
    UpdateHoverText(String),
}

pub struct TrayIface {
    sender: Sender<TrayToAppMsg>,
    receiver: Receiver<AppToTrayMsg>,
    tooltip: Mutex<String>,
}

impl AppTray {
    pub fn establish() -> anyhow::Result<Self> {
        let name = format!("org.kde.StatusNotifierItem-{}-{}", std::process::id(), 0);
        let (s1, r1) = crossbeam_channel::unbounded();
        let (s2, r2) = crossbeam_channel::unbounded();
        let conn = connection::Builder::session()?
            .name(name.clone())?
            .serve_at(
                "/StatusNotifierItem",
                TrayIface {
                    sender: s1,
                    receiver: r2,
                    tooltip: Mutex::new("mpv-frog".into()),
                },
            )?
            .build()?;
        conn.call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.kde.StatusNotifierWatcher"),
            "RegisterStatusNotifierItem",
            &name,
        )?;
        Ok(Self {
            event_flags: EventFlags::default(),
            sender: s2,
            receiver: r1,
            conn,
        })
    }
}

#[derive(Default)]
pub struct EventFlags {
    pub activated: bool,
    pub quit_clicked: bool,
    pub ctx_menu: Option<(i32, i32)>,
}

impl EventFlags {
    pub fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

type SniIcon = &'static [(i32, i32, &'static [u8])];

macro_rules! icon {
    () => {
        &[(
            32,
            32,
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/icon.argb32")),
        )]
    };
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl TrayIface {
    #[zbus(property)]
    fn category(&self) -> &'static str {
        "ApplicationStatus"
    }
    #[zbus(property)]
    fn id(&self) -> &'static str {
        "mpvfrog"
    }
    #[zbus(property)]
    fn title(&self) -> &'static str {
        "mpvfrog"
    }
    #[zbus(property)]
    fn status(&self) -> &'static str {
        "Active"
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> SniIcon {
        icon!()
    }
    #[zbus(property)]
    fn tool_tip(&self) -> (&'static str, SniIcon, String, &'static str) {
        let icon_name = "preferences-desktop-notification";
        let icon: SniIcon = &[];
        let mut tooltip = self.tooltip.lock().unwrap();
        if let Ok(msg) = self.receiver.try_recv() {
            match msg {
                AppToTrayMsg::UpdateHoverText(s) => *tooltip = s,
            }
        }
        // Unfortunately content seems to be ignored (by at least lxqt-panel)
        let content = "";
        (icon_name, icon, tooltip.clone(), content)
    }
    /// Needed so all tray providers enable "Activate"
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }
    fn context_menu(&self, x: i32, y: i32) {
        self.sender
            .send(TrayToAppMsg::ShowCtxMenu { x, y })
            .log_err("Failed to send context menu msg");
    }
    fn activate(&self, _x: i32, _y: i32) {
        self.sender
            .send(TrayToAppMsg::Activate)
            .log_err("Failed to send context menu msg");
    }
    #[zbus(signal)]
    async fn new_tool_tip(_ctx: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn new_title(_ctx: &SignalEmitter<'_>) -> zbus::Result<()>;
}
