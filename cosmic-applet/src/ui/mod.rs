//! # UI Module — All Cosmic UI components
//!
//! Contains panel_widget, main_menu, machine_detail, sensor_config, and settings_window modules.

pub mod machine_detail;
pub mod machine_list;
pub mod main_menu;
pub mod panel_widget;
pub mod sensor_config;
pub mod settings_window;

pub use panel_widget::PanelWidget;
pub use settings_window::SettingsWindow;
