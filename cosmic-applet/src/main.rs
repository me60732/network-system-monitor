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
//! pairing_manager.rs → Manages machine pairings and ECDH-derived keys for secure communication
//! udp_receiver.rs → Listens for ChaCha20-Poly1305 encrypted MetricPacket via UDP + AEAD verification
//!
//! ## Startup Sequence
//!
//! 1. Load config via [`ConfigManager`] (defaults to localhost entry).
//! 2. Initialize pairing manager for secure machine authentication.
//! 3. Initialize UDP receiver on configured port for incoming MetricPacket traffic.
//! 4. Register `PanelWidget` with the Cosmic panel — renders desktop stats in < 1s.
//! 5. On click, expand into `GridWindow` showing all remote machines.
//! 6. Background thread updates grid in real-time as UDP packets arrive.

use cosmic::Element;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::Subscription;
use cosmic::{app::Application, app::Core, app::Task};

// Module declarations
pub mod charts;
pub mod config;
pub mod i18n;
pub mod minimon_config;
pub mod network;
pub mod pairing_manager;
pub mod pairing_ui;
pub mod remote_machine;
pub mod simple_sensors;
pub mod ui;
pub mod utils;

// Import types from submodules
use crate::config::manager::ConfigManager;
use crate::ui::{PanelWidget, SettingsWindow, machine_detail, machine_list, main_menu};

/// UDP message payload types received from remote machines.
pub enum UdpPayload {
    /// A PairingRequest from an unknown sender that wants to pair.
    PairingRequest(crate::pairing_manager::PairingRequest),
}

/// UDP message wrapper for communication between receiver and UI.
pub struct UdpMessage {
    pub payload: UdpPayload,
}

// Used by the binary target; unused when compiled as the lib target for tests/benches.
#[allow(dead_code)]
const DEFAULT_CONFIG_PATH: &str = "config.toml";

/// View states for navigation — determines which UI panel is currently displayed.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    /// Panel widget - single-line toolbar with icons + compact text
    Panel,
    /// Machine list - shows all machines when 2+ machines exist
    MachineList,
    /// Main menu - sensor configuration menu
    MainMenu,
    /// Machine detail view - per-machine metric settings
    MachineDetail(String),
    /// General settings - app-wide configuration
    GeneralSettings,
    /// CPU sensor configuration
    CpuConfig,
    /// CPU Temperature sensor configuration
    CpuTempConfig,
    /// Memory sensor configuration
    MemoryConfig,
    /// Network sensor configuration
    NetworkConfig,
    /// Disk sensor configuration
    DiskConfig,
    /// GPU sensor configuration
    GpuConfig,
}

/// Message types for the Cosmic applet application.
#[derive(Debug, Clone)]
pub enum AppMessage {
    /// No operation — used when a widget needs to return a message but no action is required.
    NoOp,
    /// Navigation: open main menu from panel
    OpenMainMenu,
    /// Navigation: open machine detail view by name
    OpenMachineDetail(String),
    /// Navigation: open settings (main menu) from machine list
    OpenSettings,
    /// Navigation: open general settings
    OpenGeneralSettings,
    /// Navigation: open CPU sensor configuration
    OpenCpuConfig,
    /// Navigation: open CPU Temperature sensor configuration
    OpenCpuTempConfig,
    /// Navigation: open Memory sensor configuration
    OpenMemoryConfig,
    /// Navigation: open Network sensor configuration
    OpenNetworkConfig,
    /// Navigation: open Disk sensor configuration
    OpenDiskConfig,
    /// Navigation: open GPU sensor configuration
    OpenGpuConfig,
    /// Launch external COSMIC system monitor application
    LaunchSystemMonitor,
    /// Navigation: go back to previous view
    Back,
    /// Refresh metrics from UDP-updated RemoteMachine data
    RefreshMetrics,
    /// Remove a machine from configuration
    RemoveMachine(String),
    /// Settings window message (forwards to settings_window::SettingsMessage).
    Settings(crate::ui::settings_window::SettingsMessage),

    // Pairing system messages
    /// Received a pairing request from an unpaired machine via UDP
    PairingRequest(crate::pairing_manager::PairingRequest),
    /// Accept a pending pairing request by machine_id
    AcceptPairing(String),
    /// Deny a pending pairing request by machine_id
    DenyPairing(String),

    // CPU sensor configuration toggles
    ToggleCpuShowChart(bool),
    ToggleCpuShowValue(bool),
    ToggleCpuShowLabel(bool),
    ToggleCpuShowIcon(bool),

    // CPU Temperature sensor configuration toggles
    ToggleCpuTempShowChart(bool),
    ToggleCpuTempShowValue(bool),
    ToggleCpuTempShowLabel(bool),
    ToggleCpuTempShowIcon(bool),

    // Memory sensor configuration toggles
    ToggleMemoryShowChart(bool),
    ToggleMemoryShowAllocated(bool),
    ToggleMemoryShowValue(bool),
    ToggleMemoryShowLabel(bool),
    ToggleMemoryShowIcon(bool),
    ToggleMemoryAsPercentage(bool),

    // Network sensor configuration toggles
    ToggleNetworkCombine(bool),
    ToggleNetworkShowLabel(bool),
    ToggleNetworkShowIcon(bool),
    ToggleNetworkShowChart(bool),
    ToggleNetworkShowValue(bool),

    // Disk sensor configuration toggles
    ToggleDiskCombine(bool),
    ToggleDiskShowLabel(bool),
    ToggleDiskShowIcon(bool),
    ToggleDiskWriteShowChart(bool),
    ToggleDiskWriteShowValue(bool),
    ToggleDiskReadShowChart(bool),
    ToggleDiskReadShowValue(bool),

    // GPU sensor configuration toggles
    ToggleGpuShowLabel(bool),
    ToggleGpuShowIcon(bool),
    ToggleGpuLoadShowChart(bool),
    ToggleGpuLoadShowValue(bool),
    ToggleGpuVramShowChart(bool),
    ToggleGpuVramAsPercentage(bool),
    ToggleGpuTempShowChart(bool),
    ToggleGpuTempShowValue(bool),
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
    /// Currently visible view state - determines which UI panel is displayed
    pub current_view: View,
    /// Settings window for general configuration (always created during init)
    pub settings_window: SettingsWindow,
    /// Remote machines with live metric data (HashMap<machine_name, RemoteMachine>)
    pub machines: std::collections::HashMap<String, crate::remote_machine::RemoteMachine>,
    /// PairingManager manages paired machines and their ECDH-derived shared keys
    pub pairing_manager: std::sync::Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>>,
    /// In-memory queue of pending pairing requests waiting for user approval (60-second timeout)
    pub pending_pairings: Vec<crate::pairing_manager::PairingRequest>,
}

impl AppState {
    /// Create PairingManager with config path at ~/.config/cosmic-applet/pairing.toml
    fn create_pairing_manager()
    -> std::sync::Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_path = std::path::PathBuf::from(home)
            .join(".config")
            .join("cosmic-applet")
            .join("pairing.toml");
        std::sync::Arc::new(std::sync::RwLock::new(
            crate::pairing_manager::PairingManager::new(config_path),
        ))
    }

    /// Load MinimonConfig from COSMIC's config system, or use default if not found.
    fn load_minimon_config() -> crate::minimon_config::MinimonConfig {
        match cosmic::cosmic_config::Config::new("com.cosmic.network_system_monitor", 1) {
            Ok(config) => match crate::minimon_config::MinimonConfig::get_entry(&config) {
                Ok(loaded_config) => {
                    log::info!("Loaded MinimonConfig from COSMIC config system");
                    loaded_config
                }
                Err(e) => {
                    log::warn!("Failed to load MinimonConfig: {:?} — using defaults", e);
                    crate::minimon_config::MinimonConfig::default()
                }
            },
            Err(e) => {
                log::warn!("Failed to open COSMIC config: {:?} — using defaults", e);
                crate::minimon_config::MinimonConfig::default()
            }
        }
    }

    /// Save MinimonConfig to COSMIC's config system.
    fn save_minimon_config(config: &crate::minimon_config::MinimonConfig) {
        match cosmic::cosmic_config::Config::new("com.cosmic.network_system_monitor", 1) {
            Ok(mut cosmic_config) => {
                if let Err(e) = config.write_entry(&mut cosmic_config) {
                    log::error!("Failed to save MinimonConfig: {}", e);
                } else {
                    log::info!("Saved MinimonConfig to COSMIC config system");
                }
            }
            Err(e) => {
                log::error!("Failed to open COSMIC config for saving: {}", e);
            }
        }
    }

    /// Create AppState with fake debug data (Pluto, Saturn machines with random metrics).
    pub fn new_debug() -> Self {
        use crate::config::manager::MachineConfig;
        use crate::remote_machine::RemoteMachine;

        let mut config = ConfigManager::default();

        // Add second debug machine called "neptune"
        config.machines.push(MachineConfig::new(
            "Pluto".to_string(),
            "192.168.1.100".to_string(),
        ));

        let config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>> =
            std::sync::Arc::new(std::sync::RwLock::new(config.clone()));
        let settings_window_config = config_manager.clone();

        // Load saved MinimonConfig from COSMIC config system
        let minimon_config = Self::load_minimon_config();

        // Create RemoteMachine instances from config with fake debug data
        let config_read = config_manager.read().unwrap();
        let mut machines = std::collections::HashMap::new();
        for machine_config in &config_read.machines {
            machines.insert(
                machine_config.name.clone(),
                RemoteMachine::new_debug(machine_config.name.clone()),
            );
        }
        drop(config_read);

        let mut settings_window = SettingsWindow::new(settings_window_config);
        settings_window.update_config(minimon_config);

        let pairing_manager = Self::create_pairing_manager();

        AppState {
            config_manager,
            current_view: View::Panel, // Start at panel view
            settings_window,
            machines,
            pairing_manager,
            pending_pairings: Vec::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        use crate::remote_machine::RemoteMachine;

        // Try to load config from file, fall back to defaults if not found
        let config = if std::path::Path::new("config.toml").exists() {
            log::info!("📂 Loading config from config.toml");
            ConfigManager::load("config.toml")
        } else {
            log::info!("📂 Using default config (no config.toml found)");
            ConfigManager::default()
        };

        let config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>> =
            std::sync::Arc::new(std::sync::RwLock::new(config.clone()));
        let settings_window_config = config_manager.clone();

        // Load saved MinimonConfig from COSMIC config system
        let minimon_config = Self::load_minimon_config();

        // Create RemoteMachine instances from config
        let config_read = config_manager.read().unwrap();
        let mut machines = std::collections::HashMap::new();
        for machine_config in &config_read.machines {
            machines.insert(
                machine_config.name.clone(),
                RemoteMachine::new(machine_config.name.clone()),
            );
        }
        drop(config_read);

        let mut settings_window = SettingsWindow::new(settings_window_config);
        settings_window.update_config(minimon_config);

        let pairing_manager = Self::create_pairing_manager();

        AppState {
            config_manager,
            current_view: View::Panel, // Default to panel view
            settings_window,
            machines,
            pairing_manager,
            pending_pairings: Vec::new(),
        }
    }
}

/// Entry point — registers the application with Cosmic's panel system.
fn main() -> Result<(), cosmic::iced::Error> {
    // Initialize logging to file
    use env_logger::Builder;
    use std::fs::OpenOptions;

    let log_file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open("cosmic-applet.log")
        .expect("Failed to open log file");

    Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    log::info!("========== cosmic-applet starting ==========");

    // Parse command-line arguments and locate config file.
    let args: Vec<String> = std::env::args().collect();

    // Check for --test flag for development mode
    let test_mode = args.contains(&"--test".to_string());
    // Check for --debug flag for fake data mode
    let debug_mode = args.contains(&"--debug".to_string());

    let config_path = args
        .iter()
        .skip(1)
        .find(|arg| !arg.starts_with("--"))
        .map(|s| s.to_string())
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());

    if test_mode || debug_mode {
        if debug_mode {
            log::info!("🎲 DEBUG MODE: Running with fake data (Pluto, Saturn, localhost)");
            log::info!("Usage: cargo run -- --debug [--test]");
            unsafe {
                std::env::set_var("COSMIC_APPLET_DEBUG", "1");
            }
        }
        if test_mode {
            log::info!("🧪 DEVELOPMENT MODE: Running in standalone window for testing");
            log::info!("Usage: cargo run -- --test [--debug]");
        }
        log::info!("cosmic-applet starting — config: {}", config_path);
        log::info!("This shows the panel widget in a normal window for development");

        // Larger window for settings testing if window is clicked
        // (settings window is 700x500, so use 1000x600 to accommodate)
        let window_size = cosmic::iced::Size::new(1000.0, 600.0);
        log::info!(
            "Window size: {}x{} (tall enough for settings window)",
            window_size.width,
            window_size.height
        );

        // Launch as a regular COSMIC application with proper window
        cosmic::app::run::<PanelApplet>(
            cosmic::app::Settings::default()
                .size(window_size)
                .exit_on_close(true),
            (),
        )
    } else {
        log::info!(
            "cosmic-applet starting in PANEL MODE — config: {}",
            config_path
        );
        // Launch as a COSMIC applet — requires implementing cosmic::Application (done above).
        cosmic::applet::run::<PanelApplet>(())
    }
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
        // Check if we're in debug mode (set via environment variable from main)
        let debug_mode = std::env::var("COSMIC_APPLET_DEBUG").is_ok();

        // Create shared AppState with default or debug values.
        let app_state = if debug_mode {
            AppState::new_debug()
        } else {
            AppState::default()
        };

        // Clone pairing manager before moving app_state into shared_state
        let pairing_manager_clone = app_state.pairing_manager.clone();

        // Wrap in Arc<RwLock> ONCE — both UI and UDP receiver share this instance
        let shared_state = std::sync::Arc::new(std::sync::RwLock::new(app_state));

        if !debug_mode {
            // Clone the Arc reference (not the data!) for the thread
            let state_clone = std::sync::Arc::clone(&shared_state);

            // Spawn UDP receiver in a background task — updates app state in real-time.
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(async move {
                    crate::network::udp_receiver::UdpReceiver::start_listening_with_pairing(
                        state_clone,
                        pairing_manager_clone,
                    )
                    .await;
                });
            });
        } else {
            log::info!("🎲 DEBUG MODE: Skipping UDP receiver, using fake data");
        }

        // Initialize AppState with shared state (settings_window already created in default).
        (
            PanelApplet {
                core,
                shared_state, // Use the same Arc instance
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            AppMessage::NoOp => {
                // No operation — do nothing.
                Task::none()
            }
            AppMessage::LaunchSystemMonitor => {
                // Launch external system monitor with fallback chain.
                std::thread::spawn(|| {
                    // Try COSMIC system monitor first
                    let cosmic_commands = ["cosmic-monitor", "cosmic-comp-config system-monitor"];

                    for cmd in &cosmic_commands {
                        let parts: Vec<&str> = cmd.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }

                        let result = std::process::Command::new(parts[0])
                            .args(&parts[1..])
                            .spawn();

                        if result.is_ok() {
                            log::info!("Launched system monitor: {}", cmd);
                            return;
                        }
                    }

                    // Fallback to generic system monitors
                    let fallback_commands = ["gnome-system-monitor", "ksysguard", "htop"];

                    for cmd in &fallback_commands {
                        let result = std::process::Command::new(cmd).spawn();
                        if result.is_ok() {
                            log::info!("Launched fallback system monitor: {}", cmd);
                            return;
                        }
                    }

                    log::warn!("Failed to launch any system monitor — no supported command found");
                });

                Task::none()
            }
            AppMessage::RefreshMetrics => {
                // Periodic refresh - just trigger a view update by returning Task::none().
                // The view() function will read the latest data from shared_state.
                let state = self.shared_state.read().unwrap();
                if let Some((name, machine)) = state.machines.iter().next() {
                    log::debug!(
                        "🔄 RefreshMetrics: machine '{}' CPU={:.1}%, mem={}/{}",
                        name,
                        machine.sensors.cpu.usage_percent,
                        machine.sensors.memory.used_bytes,
                        machine.sensors.memory.total_bytes
                    );
                }
                drop(state);

                // Prune expired pairing requests (60-second timeout)
                let mut state = self.shared_state.write().unwrap();
                state
                    .pending_pairings
                    .retain(|r| r.received_at.elapsed().as_secs() < 60);

                Task::none()
            }
            AppMessage::OpenMainMenu => {
                // Navigate to main menu or machine list depending on machine count.
                let mut app_state = self.shared_state.write().unwrap();
                let machine_count = app_state.machines.len();
                app_state.current_view = if machine_count >= 2 {
                    View::MachineList
                } else {
                    View::MainMenu
                };
                Task::none()
            }
            AppMessage::OpenMachineDetail(machine_name) => {
                // Open machine detail view by name.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::MachineDetail(machine_name);
                Task::none()
            }
            AppMessage::OpenSettings => {
                // Navigate to main menu (settings) from machine list.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::MainMenu;
                Task::none()
            }
            AppMessage::OpenGeneralSettings => {
                // Navigate to general settings.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::GeneralSettings;
                Task::none()
            }
            AppMessage::OpenCpuConfig => {
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::CpuConfig;
                Task::none()
            }
            AppMessage::OpenCpuTempConfig => {
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::CpuTempConfig;
                Task::none()
            }
            AppMessage::OpenMemoryConfig => {
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::MemoryConfig;
                Task::none()
            }
            AppMessage::OpenNetworkConfig => {
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::NetworkConfig;
                Task::none()
            }
            AppMessage::OpenDiskConfig => {
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::DiskConfig;
                Task::none()
            }
            AppMessage::OpenGpuConfig => {
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::GpuConfig;
                Task::none()
            }
            AppMessage::Back => {
                // Go back to previous view.
                let mut app_state = self.shared_state.write().unwrap();
                let machine_count = app_state.machines.len();
                app_state.current_view = match app_state.current_view {
                    View::MachineList => View::Panel,
                    View::MainMenu | View::GeneralSettings => {
                        if machine_count >= 2 {
                            View::MachineList
                        } else {
                            View::Panel
                        }
                    }
                    View::MachineDetail(_) => {
                        if machine_count >= 2 {
                            View::MachineList
                        } else {
                            View::Panel
                        }
                    }
                    _ => View::MainMenu,
                };
                Task::none()
            }
            AppMessage::RemoveMachine(machine_name) => {
                // Remove a machine from configuration.
                let mut app_state = self.shared_state.write().unwrap();
                app_state
                    .config_manager
                    .write()
                    .unwrap()
                    .machines
                    .retain(|m| m.name != machine_name);
                let _ = app_state.config_manager.write().unwrap().save();

                // Return to main menu after removal
                app_state.current_view = View::MainMenu;
                Task::none()
            }
            AppMessage::PairingRequest(request) => {
                let mut state = self.shared_state.write().unwrap();
                // Deduplicate: only add if not already pending
                if !state
                    .pending_pairings
                    .iter()
                    .any(|r| r.machine_id == request.machine_id)
                {
                    log::info!(
                        "🔔 New pairing request from machine: {}",
                        request.machine_id
                    );
                    state.pending_pairings.push(request);
                }
                Task::none()
            }
            AppMessage::AcceptPairing(machine_id) => {
                let mut state = self.shared_state.write().unwrap();
                if let Some(idx) = state
                    .pending_pairings
                    .iter()
                    .position(|r| r.machine_id == machine_id)
                {
                    let req = state.pending_pairings.remove(idx);
                    // Derive ECDH shared key and persist
                    let mut pm = state.pairing_manager.write().unwrap();
                    if let Err(e) =
                        pm.add_pairing(req.machine_id.clone(), &req.sender_pubkey, req.host.clone())
                    {
                        log::error!("Failed to persist pairing for {}: {}", req.machine_id, e);
                    } else {
                        log::info!("✅ Pairing accepted for machine: {}", req.machine_id);
                    }
                }
                Task::none()
            }
            AppMessage::DenyPairing(machine_id) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .pending_pairings
                    .retain(|r| r.machine_id != machine_id);
                log::info!("❌ Pairing denied for machine: {}", machine_id);
                Task::none()
            }
            AppMessage::Settings(settings_message) => {
                // Forward settings window message to settings_window handler.
                let mut app_state = self.shared_state.write().unwrap();

                match settings_message {
                    crate::ui::settings_window::SettingsMessage::CloseWindow => {
                        app_state.current_view = View::MainMenu;
                    }
                    crate::ui::settings_window::SettingsMessage::UpdateRefreshRate(seconds) => {
                        app_state.settings_window.minimon_config.refresh_rate =
                            (seconds * 1000.0) as u32;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::IncrementRefreshRate => {
                        let current =
                            app_state.settings_window.minimon_config.refresh_rate as f64 / 1000.0;
                        let new_val = (current + 0.1).min(10.0);
                        app_state.settings_window.minimon_config.refresh_rate =
                            (new_val * 1000.0) as u32;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::DecrementRefreshRate => {
                        let current =
                            app_state.settings_window.minimon_config.refresh_rate as f64 / 1000.0;
                        let new_val = (current - 0.1).max(0.1);
                        app_state.settings_window.minimon_config.refresh_rate =
                            (new_val * 1000.0) as u32;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::UpdateValueSize(size) => {
                        app_state.settings_window.minimon_config.value_size_default = size;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::IncrementValueSize => {
                        let current = app_state.settings_window.minimon_config.value_size_default;
                        app_state.settings_window.minimon_config.value_size_default =
                            (current + 1).min(24);
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::DecrementValueSize => {
                        let current = app_state.settings_window.minimon_config.value_size_default;
                        app_state.settings_window.minimon_config.value_size_default =
                            (current.saturating_sub(1)).max(8);
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleMonospace(enabled) => {
                        app_state.settings_window.minimon_config.monospace_values = enabled;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::UpdatePanelSpacing(spacing) => {
                        app_state.settings_window.minimon_config.panel_spacing = spacing;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::MoveContentUp(idx) => {
                        if idx > 0 {
                            app_state
                                .settings_window
                                .minimon_config
                                .content_order
                                .order
                                .swap(idx, idx - 1);
                            AppState::save_minimon_config(
                                &app_state.settings_window.minimon_config,
                            );
                        }
                    }
                    crate::ui::settings_window::SettingsMessage::MoveContentDown(idx) => {
                        let len = app_state
                            .settings_window
                            .minimon_config
                            .content_order
                            .order
                            .len();
                        if idx < len - 1 {
                            app_state
                                .settings_window
                                .minimon_config
                                .content_order
                                .order
                                .swap(idx, idx + 1);
                            AppState::save_minimon_config(
                                &app_state.settings_window.minimon_config,
                            );
                        }
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleCpuVisible(visible) => {
                        app_state.settings_window.minimon_config.sensor_cpu_visible = visible;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleCpuTempVisible(visible) => {
                        app_state
                            .settings_window
                            .minimon_config
                            .sensor_cpu_temp_visible = visible;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleMemoryVisible(visible) => {
                        app_state
                            .settings_window
                            .minimon_config
                            .sensor_memory_visible = visible;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleNetworkVisible(visible) => {
                        app_state
                            .settings_window
                            .minimon_config
                            .sensor_network_visible = visible;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleDiskVisible(visible) => {
                        app_state.settings_window.minimon_config.sensor_disk_visible = visible;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::ToggleGpuVisible(visible) => {
                        app_state.settings_window.minimon_config.sensor_gpu_visible = visible;
                        AppState::save_minimon_config(&app_state.settings_window.minimon_config);
                    }
                    crate::ui::settings_window::SettingsMessage::NoOp => {
                        // No operation — do nothing.
                    }
                }

                Task::none()
            }

            // CPU sensor configuration toggles
            AppMessage::ToggleCpuShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state.settings_window.minimon_config.cpu.show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleCpuShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state.settings_window.minimon_config.cpu.show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleCpuShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state.settings_window.minimon_config.cpu.show_label(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleCpuShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state.settings_window.minimon_config.cpu.show_icon(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }

            // CPU Temperature sensor configuration toggles
            AppMessage::ToggleCpuTempShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .cputemp
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleCpuTempShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .cputemp
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleCpuTempShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .cputemp
                    .show_label(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleCpuTempShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .cputemp
                    .show_icon(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }

            // Memory sensor configuration toggles
            AppMessage::ToggleMemoryShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .memory
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleMemoryShowAllocated(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state.settings_window.minimon_config.memory.show_allocated = enabled;
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleMemoryShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .memory
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleMemoryShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .memory
                    .show_label(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleMemoryShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .memory
                    .show_icon(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleMemoryAsPercentage(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state.settings_window.minimon_config.memory.percentage = enabled;
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }

            // Network sensor configuration toggles
            AppMessage::ToggleNetworkCombine(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                if enabled {
                    state.settings_window.minimon_config.network1.variant =
                        crate::minimon_config::NetworkVariant::Combined;
                } else {
                    state.settings_window.minimon_config.network1.variant =
                        crate::minimon_config::NetworkVariant::Download;
                }
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleNetworkShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .network1
                    .show_label(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleNetworkShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .network1
                    .show_icon(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleNetworkShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .network1
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleNetworkShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .network1
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }

            // Disk sensor configuration toggles
            AppMessage::ToggleDiskCombine(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                if enabled {
                    state.settings_window.minimon_config.disks1.variant =
                        crate::minimon_config::DisksVariant::Combined;
                } else {
                    state.settings_window.minimon_config.disks1.variant =
                        crate::minimon_config::DisksVariant::Write;
                }
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleDiskShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .disks1
                    .show_label(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleDiskShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .disks1
                    .show_icon(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleDiskWriteShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .disks1
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleDiskWriteShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .disks1
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleDiskReadShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .disks2
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleDiskReadShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .disks2
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }

            // GPU sensor configuration toggles
            AppMessage::ToggleGpuShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .usage
                    .show_label(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .usage
                    .show_icon(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuLoadShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .usage
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuLoadShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .usage
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuVramShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .vram
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuVramAsPercentage(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .vram
                    .percentage = enabled;
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuTempShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .temp
                    .show_chart(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
            AppMessage::ToggleGpuTempShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                state
                    .settings_window
                    .minimon_config
                    .gpus
                    .entry("default".to_string())
                    .or_default()
                    .temp
                    .show_value(enabled);
                AppState::save_minimon_config(&state.settings_window.minimon_config);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        // Copy state before rendering to avoid lifetime issues.
        let (config, current_view, settings_visible, pending_count, pending_pairings_clone) = {
            let state = self.shared_state.read().unwrap();
            let config_guard = state.config_manager.read().unwrap();
            let config = config_guard.clone();

            let current_view = state.current_view.clone();
            let settings_visible = state.settings_window.visible;
            let pending_count = state.pending_pairings.len();
            let pending_pairings_clone = state.pending_pairings.clone();

            (
                config,
                current_view,
                settings_visible,
                pending_count,
                pending_pairings_clone,
            )
        };

        // Pairing UI hijack: when pairings are pending AND user opens the dropdown, show pairing view.
        if pending_count > 0 && current_view != View::Panel {
            return crate::pairing_ui::view(&pending_pairings_clone);
        }

        // Render based on current UI state — SettingsWindow overlays other views.
        if settings_visible {
            // Show SettingsWindow when visible (overlay mode).
            let state = self.shared_state.read().unwrap();
            let minimon_config = state.settings_window.minimon_config.clone();
            drop(state);

            return crate::ui::settings_window::view_with_config(&minimon_config)
                .map(|msg| AppMessage::Settings(msg));
        }

        // Render view based on current_view state
        match current_view {
            View::Panel => {
                // Show panel widget with real machine data
                let state = self.shared_state.read().unwrap();
                let machines_vec: Vec<_> = state.machines.values().cloned().collect();
                let content_order = state.settings_window.minimon_config.content_order.clone();
                let minimon_config = state.settings_window.minimon_config.clone();
                let pending_count = state.pending_pairings.len();
                drop(state);

                // Debug: log the data we're about to render
                if let Some(machine) = machines_vec.first() {
                    log::debug!(
                        "🖼️  Rendering Panel: machine '{}' CPU={:.1}%, mem={}/{}",
                        machine.name,
                        machine.sensors.cpu.usage_percent,
                        machine.sensors.memory.used_bytes,
                        machine.sensors.memory.total_bytes
                    );
                }

                let panel =
                    PanelWidget::view_from_machines(&machines_vec, &content_order, &minimon_config);

                if pending_count > 0 {
                    // Wrap panel widget with a badge indicator
                    cosmic::widget::row(vec![
                        panel,
                        crate::pairing_ui::pending_badge(pending_count),
                    ])
                    .into()
                } else {
                    panel
                }
            }
            View::MachineList => {
                // Show machine list with all machines
                let state = self.shared_state.read().unwrap();
                let machines_vec: Vec<_> = state.machines.values().cloned().collect();
                let content_order = state.settings_window.minimon_config.content_order.clone();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                machine_list::view(&machines_vec, &content_order, &minimon_config)
            }
            View::MainMenu => {
                // Show main menu with machine data
                let state = self.shared_state.read().unwrap();
                let machines_vec: Vec<_> = state.machines.values().cloned().collect();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                main_menu::view(
                    &self.shared_state.read().unwrap().config_manager,
                    &machines_vec,
                    &minimon_config,
                )
            }
            View::MachineDetail(ref machine_name) => {
                // Show machine detail view with live data
                let state = self.shared_state.read().unwrap();
                if let Some(remote_machine) = state.machines.get(machine_name) {
                    if let Some(config) = config.machines.iter().find(|m| m.name == *machine_name) {
                        let minimon_config = state.settings_window.minimon_config.clone();
                        machine_detail::view(config, remote_machine, &minimon_config)
                    } else {
                        cosmic::widget::button::text("← Back")
                            .on_press(AppMessage::Back)
                            .into()
                    }
                } else {
                    cosmic::widget::button::text("← Back")
                        .on_press(AppMessage::Back)
                        .into()
                }
            }
            View::GeneralSettings => {
                // Show general settings
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);

                crate::ui::settings_window::view_with_config(&minimon_config)
                    .map(|msg| AppMessage::Settings(msg))
            }
            View::CpuConfig => {
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                crate::ui::sensor_config::view_cpu_config(&minimon_config)
            }
            View::CpuTempConfig => {
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                crate::ui::sensor_config::view_cpu_temp_config(&minimon_config)
            }
            View::MemoryConfig => {
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                crate::ui::sensor_config::view_memory_config(&minimon_config)
            }
            View::NetworkConfig => {
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                crate::ui::sensor_config::view_network_config(&minimon_config)
            }
            View::DiskConfig => {
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                crate::ui::sensor_config::view_disk_config(&minimon_config)
            }
            View::GpuConfig => {
                let state = self.shared_state.read().unwrap();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);
                crate::ui::sensor_config::view_gpu_config(&minimon_config)
            }
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Refresh at 1Hz — matches the nmd-service send rate so there's no point
        // re-rendering faster than data actually arrives. Reducing from 500ms to 1000ms
        // halves the number of expensive view() re-renders (canvas ring charts) during scroll.
        cosmic::iced::time::every(std::time::Duration::from_millis(1000))
            .map(|_| AppMessage::RefreshMetrics)
    }
}
