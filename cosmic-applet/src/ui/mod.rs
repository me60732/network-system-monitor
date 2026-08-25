//! User interface components for the cosmic-applet.
//!
//! Contains panel widget, grid window, machine cards, and settings UI.

pub mod panel_widget;
pub mod grid_window;
pub mod machine_card;
pub mod settings_window;
pub mod machine_row;

pub use panel_widget::PanelWidget;
pub use grid_window::GridWindow;
pub use machine_card::MachineCard;
pub use settings_window::SettingsWindow;
pub use machine_row::{MachineRow, MachineStatus};

// Note: The original ui/mod.rs had a helper function `labeled_value` which we are removing for simplicity.
// If needed, it can be added back later.