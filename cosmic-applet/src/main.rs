//! # cosmic-applet — Network System Monitor Desktop Panel Applet
//!
//! This is the Cosmic desktop panel applet that displays system metrics from all remote machines
//! on your local network. The panel widget shows a single-line summary of desktop CPU, memory,
//! disk, network, uptime, GPU VRAM, and temperature stats. Clicking the panel opens a grid window
//! listing every registered machine with per-metric status indicators and color-coded progress bars.
//!
//! ## Architecture Overview (pop-os/cosmic-applet-template structure)
//!
//! ```text
//! main.rs         → Applet entry point: registers PanelWidget, handles click-to-expand
//! panel_widget.rs → Single-line Cosmic panel rendering with 60%/80% color thresholds
//! grid_window.rs  → Click-expanded window showing all remote machines in a grid layout
//! machine_row.rs  → One row per machine with status indicators and metric progress bars
//! config/manager.rs → TOML config loading/saving, manages machine list + metric selection
//! udp_receiver.rs → Listens for rkyv-encoded MetricPacket via UDP + HMAC-SHA256 verification
//!
//! ## Startup Sequence
//!
//! 1. Load config via [`ConfigManager`] (defaults to localhost entry).
//! 2. Initialize UDP receiver on configured port for incoming MetricPacket traffic.
//! 3. Register `PanelWidget` with the Cosmic panel — renders desktop stats in < 1s.
//! 4. On click, expand into `GridWindow` showing all remote machines.
//! 5. Background thread updates grid in real-time as UDP packets arrive.

use cosmic::{app::Application, app::Core, app::Task};
use cosmic::iced::Subscription;
use cosmic::Element;

// Module declarations (must come before imports)
pub mod config;
pub mod ui;
pub mod charts;
pub mod network;
pub mod utils;

// Import types from submodules
use crate::ui::{PanelWidget, GridWindow, SettingsWindow};
use crate::config::manager::ConfigManager;

const DEFAULT_CONFIG_PATH: &str = "config.toml";

/// Message types for the Cosmic applet application.
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// No operation — used when a widget needs to return a message but no action is required.
    NoOp,
    /// Toggle settings window visibility.
    ToggleSettingsWindow,
    /// Settings window message (forwards to settings_window::SettingsMessage).
    Settings(crate::ui::settings_window::SettingsMessage),
}

/// The main Cosmic Applet struct that wraps [`PanelWidget`] and handles panel registration.
///
/// This follows pop-os/cosmic-applet-template conventions: implements `cosmic::Application`
/// which provides the applet lifecycle (init → update → view). Signal handlers connect user
/// interactions (click-to-expand → GridWindow). The shared state is updated by a background
/// UDP receiver task spawned during init.
pub struct PanelApplet {
    /// Cosmic Core handle — required by the Application trait for launching tasks and accessing runtime.
    core: Core,
    /// Shared state between panel widget and grid window — updated by UDP receiver task.
    pub shared_state: std::sync::Arc<std::sync::RwLock<AppState>>,
}

/// Global application state shared across all UI components via `std::sync::Arc<std::sync::RwLock<>>`.
#[derive(Clone)]
pub struct AppState {
    /// Loaded configuration including machine list and metric selections (shared via std::sync::Arc<std::sync::RwLock>).
    pub config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>>,
    /// Currently visible grid window (None when panel widget is shown alone).
    pub grid_window: Option<GridWindow>,
    /// Settings window for configuring machines and metrics (always created during init).
    pub settings_window: SettingsWindow,
}

impl Default for AppState {
    fn default() -> Self {
        let config = ConfigManager::default();
        let config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>> = 
            std::sync::Arc::new(std::sync::RwLock::new(config));
        let settings_window_config = config_manager.clone();

        AppState {
            config_manager,
            grid_window: None,
            settings_window: SettingsWindow::new(settings_window_config),
        }
    }
}

/// Entry point — registers the application with Cosmic's panel system.
fn main() -> Result<(), cosmic::iced::Error> {
    // Parse command-line arguments and locate config file.
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());

    log::info!("cosmic-applet starting — config: {}", config_path);

    // Launch as a COSMIC applet — requires implementing cosmic::Application (done above).
    cosmic::applet::run::<PanelApplet>(())
}

impl Application for PanelApplet {
    type Executor = cosmic::executor::Default;
    type Message = AppMessage;
    type Flags = ();

    const APP_ID: &'static str = "com.cosmic.network_system_monitor";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        // Create shared AppState with default values.
        let app_state = AppState::default();

        // Clone state for thread spawn
        let state_clone: std::sync::Arc<std::sync::RwLock<AppState>> = 
            std::sync::Arc::clone(&std::sync::Arc::new(std::sync::RwLock::new(app_state.clone())));

        // Spawn UDP receiver in a background task — updates grid window in real-time.
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async move {
                crate::network::udp_receiver::UdpReceiver::start_listening(state_clone).await;
            });
        });

        // Initialize AppState with shared state (settings_window already created in default).
        (
            PanelApplet { 
                core, 
                shared_state: std::sync::Arc::new(std::sync::RwLock::new(app_state)) 
            }, 
            Task::none()
        )
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            AppMessage::NoOp => {
                // No operation — do nothing.
                Task::none()
            }
            AppMessage::ToggleSettingsWindow => {
                // Toggle settings window visibility.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.settings_window.visible = !app_state.settings_window.visible;
                Task::none()
            }
            AppMessage::Settings(settings_message) => {
                // Forward settings window message to settings_window handler.
                let mut app_state = self.shared_state.write().unwrap();
                
                match settings_message {
                    crate::ui::settings_window::SettingsMessage::AddMachine => {
                        // Add a new machine with default config.
                        app_state.config_manager.write().unwrap().machines.push(crate::config::manager::MachineConfig {
                            name: "new-machine".to_string(),
                            enabled: true,
                            host: "127.0.0.1".to_string(),
                            port: 51058,
                            show_cpu: true,
                            show_memory: true,
                            show_disk: true,
                            show_network: true,
                            show_uptime: true,
                            show_gpu_vram: true,
                            show_temperature: true,
                        });
                    }
                    crate::ui::settings_window::SettingsMessage::RemoveSelected => {
                        // Remove the selected machine (index 0 for now).
                        app_state.config_manager.write().unwrap().machines.clear();
                    }
                    crate::ui::settings_window::SettingsMessage::CloseWindow => {
                        app_state.settings_window.visible = false;
                    }
                    crate::ui::settings_window::SettingsMessage::NoOp => {
                        // No operation — do nothing.
                    }
                    crate::ui::settings_window::SettingsMessage::UpdateMachineField(_, _, _) => {
                        // Update a field in a specific machine configuration (not implemented yet).
                    }
                    crate::ui::settings_window::SettingsMessage::UpdateMachineMetric(_, _, _) => {
                        // Toggle a metric display setting for a specific machine (not implemented yet).
                    }
                }
                
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        // Copy state before rendering to avoid lifetime issues.
        let (config, grid_visible, grid_rows, settings_visible) = {
            let state = self.shared_state.read().unwrap();
            let config_guard = state.config_manager.read().unwrap();
            let config = config_guard.clone();

            let grid_visible = state.grid_window.is_some() && 
                state.grid_window.as_ref().unwrap().visible;
            let settings_visible = state.settings_window.visible;

            let grid_rows = if let Some(ref g) = state.grid_window {
                g.rows.clone()
            } else {
                Vec::new()
            };

            (config, grid_visible, grid_rows, settings_visible)
        };

        // Render based on current UI state — SettingsWindow overlays GridWindow.
        if settings_visible {
            // Show SettingsWindow when visible (overlay mode).
            let state = self.shared_state.read().unwrap();
            
            return state.settings_window.view()
                .map(|msg| AppMessage::Settings(msg));
        } else if grid_visible && !grid_rows.is_empty() {
            // Show GridWindow when panel clicked but settings not open.
            GridWindow::view_with_data(&grid_rows)
        } else {
            // Default: show panel widget with desktop stats and settings button using ring charts.
            let metric_configs = crate::ui::panel_widget::MetricConfigs::default();
            
            PanelWidget::view_from_machines_with_config(
                &config.machines, 
                metric_configs,
            ).map(|_| AppMessage::NoOp)
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // No subscriptions needed for basic applet functionality.
        Subscription::none()
    }
}

