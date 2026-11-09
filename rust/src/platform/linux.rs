//! Linux desktop integration around the X11 windows: the tray icon (StatusNotifierItem
//! over D-Bus, pure Rust via ksni/zbus) and the "Say something…" prompt (zenity/kdialog).
//! Both run off the main thread and report back through the backend's event channel.

use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use ksni::blocking::{Handle, TrayMethods};

use super::{Event, MenuItem};

struct Tray {
    icon: ksni::Icon,
    menu: Vec<MenuItem>,
    tx: Sender<Event>,
}

fn convert(items: &[MenuItem], tx: &Sender<Event>) -> Vec<ksni::MenuItem<Tray>> {
    items
        .iter()
        .map(|item| match item {
            MenuItem::Action { id, label, checked } => {
                let (id, tx) = (*id, tx.clone());
                let activate = Box::new(move |_: &mut Tray| {
                    let _ = tx.send(Event::Menu(id));
                });
                match checked {
                    Some(on) => ksni::menu::CheckmarkItem { label: label.clone(), checked: *on, activate, ..Default::default() }.into(),
                    None => ksni::menu::StandardItem { label: label.clone(), activate, ..Default::default() }.into(),
                }
            }
            MenuItem::Submenu { label, items } => {
                ksni::menu::SubMenu { label: label.clone(), submenu: convert(items, tx), ..Default::default() }.into()
            }
            MenuItem::Separator => ksni::MenuItem::Separator,
        })
        .collect()
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "desktop-mascots".into()
    }
    fn title(&self) -> String {
        "Desktop Mascots".into()
    }
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }
    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: "Desktop Mascots".into(), ..Default::default() }
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        convert(&self.menu, &self.tx)
    }
}

pub struct TrayIcon {
    handle: Handle<Tray>,
}

impl TrayIcon {
    /// None if there's no tray host (no StatusNotifierWatcher, e.g. GNOME without the extension).
    pub fn new(icon_rgba: &[u8], size: u32, menu: Vec<MenuItem>, tx: Sender<Event>) -> Option<TrayIcon> {
        // ARGB32, network byte order
        let data = icon_rgba.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
        let icon = ksni::Icon { width: size as i32, height: size as i32, data };
        let tray = Tray { icon, menu, tx };
        match tray.spawn() {
            Ok(handle) => Some(TrayIcon { handle }),
            Err(e) => {
                eprintln!("note: no tray icon ({e})");
                None
            }
        }
    }

    pub fn set_menu(&self, menu: Vec<MenuItem>) {
        self.handle.update(move |t| t.menu = menu);
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.handle.shutdown().wait();
    }
}

/// Ask for a line of text with whichever desktop dialog tool is installed; the answer
/// (if any) arrives as `Event::Said`. Doesn't block.
pub fn ask_text(tag: u32, title: &str, label: &str, tx: Sender<Event>) {
    let (title, label) = (title.to_string(), label.to_string());
    std::thread::spawn(move || {
        let attempts: [(&str, Vec<String>); 3] = [
            ("zenity", vec!["--entry".into(), format!("--title={title}"), format!("--text={label}")]),
            ("kdialog", vec!["--title".into(), title.clone(), "--inputbox".into(), label.clone()]),
            ("yad", vec!["--entry".into(), format!("--title={title}"), format!("--text={label}")]),
        ];
        for (tool, args) in attempts {
            let out = Command::new(tool).args(&args).stdin(Stdio::null()).stderr(Stdio::null()).output();
            match out {
                Err(_) => continue, // not installed: try the next one
                Ok(out) => {
                    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if out.status.success() && !text.is_empty() {
                        let _ = tx.send(Event::Said { tag, text });
                    }
                    return; // answered or cancelled
                }
            }
        }
        eprintln!("can't ask for text: install zenity or kdialog (or use: desktop-mascots say TEXT)");
    });
}
