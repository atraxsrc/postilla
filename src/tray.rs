// SPDX-License-Identifier: GPL-3.0

//! System tray icon (StatusNotifierItem over D-Bus), shown by the COSMIC
//! panel status area and by any other desktop that implements the protocol.

use cosmic::iced::Subscription;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, Tray, TrayMethods};
use tokio::sync::mpsc;

use crate::fl;

/// Something the user did on the tray icon or its menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Bring the window to the front
    Show,
    /// Close the application
    Quit,
}

struct PostillaTray {
    sender: mpsc::UnboundedSender<TrayEvent>,
    icons: Vec<Icon>,
}

impl PostillaTray {
    fn emit(&self, event: TrayEvent) {
        let _ = self.sender.send(event);
    }
}

impl Tray for PostillaTray {
    fn id(&self) -> String {
        crate::app::APP_ID.to_string()
    }

    fn title(&self) -> String {
        "Postilla".to_string()
    }

    fn icon_name(&self) -> String {
        // Used when the icon is installed in the icon theme (just install / deb)
        format!("{}-symbolic", crate::app::APP_ID)
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        // Rendered from the bundled SVG, works when running from the source tree
        self.icons.clone()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.emit(TrayEvent::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: fl!("tray-show"),
                activate: Box::new(|tray: &mut Self| tray.emit(TrayEvent::Show)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: fl!("tray-quit"),
                activate: Box::new(|tray: &mut Self| tray.emit(TrayEvent::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Registers the tray icon and streams its events to the application.
pub fn subscription() -> Subscription<TrayEvent> {
    use cosmic::iced::futures::SinkExt;

    use cosmic::iced::futures::channel::mpsc as iced_mpsc;

    Subscription::run(|| {
        cosmic::iced::stream::channel(8, |mut output: iced_mpsc::Sender<TrayEvent>| async move {
            let (sender, mut receiver) = mpsc::unbounded_channel();

            let tray = PostillaTray {
                sender,
                icons: render_icons(),
            };

            let handle = match tray.spawn().await {
                Ok(handle) => handle,
                Err(err) => {
                    eprintln!("tray icon unavailable (no StatusNotifier host?): {err}");
                    // keep the subscription alive without a tray
                    std::future::pending::<()>().await;
                    return;
                }
            };

            while let Some(event) = receiver.recv().await {
                let _ = output.send(event).await;
            }

            drop(handle);
        })
    })
}

const ICON_SVG: &[u8] = include_bytes!("../resources/icons/hicolor/symbolic/apps/icon-symbolic.svg");

/// Rasterizes the symbolic application icon at the sizes panels commonly use,
/// tinted with the text color of the active theme so it blends in with the
/// other panel icons.
fn render_icons() -> Vec<Icon> {
    let tree = match resvg::usvg::Tree::from_data(ICON_SVG, &resvg::usvg::Options::default()) {
        Ok(tree) => tree,
        Err(err) => {
            eprintln!("tray icon: could not parse the app icon: {err}");
            return Vec::new();
        }
    };

    let tint = cosmic::theme::active().cosmic().on_bg_color();
    let (tr, tg, tb) = (
        (tint.red * 255.0).round() as u8,
        (tint.green * 255.0).round() as u8,
        (tint.blue * 255.0).round() as u8,
    );

    [22u32, 32, 48, 64]
        .into_iter()
        .filter_map(|size| {
            let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
            let scale = size as f32 / tree.size().width().max(tree.size().height());
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );

            // StatusNotifierItem wants ARGB32 in network byte order, not
            // premultiplied. Only the shape (alpha) of the icon is kept, the
            // color is the theme's.
            let data = pixmap
                .pixels()
                .iter()
                .flat_map(|px| [px.demultiply().alpha(), tr, tg, tb])
                .collect();

            Some(Icon {
                width: size as i32,
                height: size as i32,
                data,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_icon_rasterizes() {
        let icons = render_icons();
        assert_eq!(icons.len(), 4);
        for icon in &icons {
            assert_eq!(icon.data.len(), (icon.width * icon.height * 4) as usize);
            // the glyph must actually paint something
            let painted = icon.data.chunks(4).filter(|px| px[0] > 0).count();
            assert!(painted > (icon.width * icon.height / 20) as usize);
        }
    }
}
