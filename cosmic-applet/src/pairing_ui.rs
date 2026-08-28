//! # pairing_ui — Bluetooth-style TOFU pairing request UI for the Cosmic panel applet
//!
//! Renders the dual-mode dropdown hijack when unknown machines request to connect.
//! Mode 1 (normal) = standard menu (handled by main_menu.rs).
//! Mode 2 (pairing pending) = this module takes over and shows Accept/Deny per machine.

use crate::AppMessage;
use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{button, column, container, divider, row, scrollable, text};

/// Render the pairing requests panel.
/// Called by main.rs `view()` when `pending_pairings.len() > 0`.
pub fn view(
    pending_pairings: &[crate::pairing_manager::PairingRequest],
) -> Element<'static, AppMessage> {
    // Header row with 🔔 icon and title
    let header_row = row(vec![
        text("🔔").size(16).into(),
        text("New machines requesting access:")
            .size(14)
            .width(Length::Fill)
            .into(),
    ])
    .spacing(8)
    .align_y(Alignment::Center);

    // Collect all content elements into a single Vec for the column
    let mut content_items: Vec<Element<'static, AppMessage>> = vec![
        container(header_row)
            .padding([12, 16])
            .width(Length::Fill)
            .into(),
        divider::horizontal::default().into(),
    ];

    if pending_pairings.is_empty() {
        content_items.push(
            container(text("No pending requests").size(12))
                .padding([16, 16])
                .width(Length::Fill)
                .into(),
        );
    } else {
        for (idx, req) in pending_pairings.iter().enumerate() {
            if idx > 0 {
                content_items.push(divider::horizontal::default().into());
            }

            let entry_row = row(vec![
                text("🔌").size(16).width(Length::Fixed(28.0)).into(),
                text(format!("{} ({})", req.machine_id, req.host))
                    .size(14)
                    .width(Length::Fill)
                    .into(),
                button::text("Accept")
                    .on_press(AppMessage::AcceptPairing(req.machine_id.clone()))
                    .into(),
                button::text("Deny")
                    .on_press(AppMessage::DenyPairing(req.machine_id.clone()))
                    .into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center);

            content_items.push(
                container(entry_row)
                    .padding([12, 16])
                    .width(Length::Fill)
                    .into(),
            );
        }
    }

    // Footer note about expiration
    content_items.push(divider::horizontal::default().into());
    content_items.push(
        container(text("Requests expire after 60 seconds").size(12))
            .padding([8, 16])
            .width(Length::Fill)
            .into(),
    );

    let content = column(content_items).spacing(0);

    let scrollable_content = scrollable(content).height(Length::Shrink);

    container(scrollable_content)
        .width(Length::Fixed(430.0))
        .max_height(600.0)
        .into()
}

/// Render a small badge/notification indicator for the panel widget.
/// Returns a text element showing the pending count, or empty text when none.
pub fn pending_badge(count: usize) -> Element<'static, AppMessage> {
    if count == 0 {
        text("").into()
    } else {
        text(format!("🔔 {}", count)).into()
    }
}
