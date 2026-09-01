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
use cosmic::iced::window::Id as WindowId;
use cosmic::widget::Id as WidgetId;
use cosmic::widget::autosize;

static AUTOSIZE_MAIN_ID: LazyLock<WidgetId> = LazyLock::new(WidgetId::unique);

// PanelAnchor is private in cosmic-applet, use matches pattern directly
use cosmic::{app::Application, app::Core, app::Task};
use std::sync::LazyLock;

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
use crate::ui::SettingsWindow;
use cosmic::iced::Limits;

/// UDP message payload types received from remote machines.
pub enum UdpPayload {
    /// A PairingRequest from an unknown sender that wants to pair.
    PairingRequest(crate::pairing_manager::PairingRequest),
}

/// UDP message wrapper for communication between receiver and UI.
pub struct UdpMessage {
    pub payload: UdpPayload,
}

/// View states for navigation — determines which UI panel is currently displayed.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    /// Panel widget - single-line toolbar with icons + compact text
    Panel,
    /// Machine list - shows all machines when 2+ machines exist
    MachineList,
    /// Machine sensor config menu - per-machine sensor configuration
    MachineSensorConfig(String), // NEW - replaces MainMenu, carries machine name
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
    /// Copy text to clipboard.
    CopyToClipboard(String),

    /// Toggle the popup window open/closed
    TogglePopup,
    /// Popup window was closed externally
    PopupClosed(WindowId),

    // Pairing system messages
    /// Received a pairing request from an unpaired machine via UDP
    PairingRequest(crate::pairing_manager::PairingRequest),
    /// Accept a pending pairing request by machine_id
    AcceptPairing(String),
    /// Deny a pending pairing request by machine_id
    DenyPairing(String),
    /// Navigation: open machine sensor config menu for a specific machine
    OpenMachineSensorConfig(String),

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
    /// Popup window ID when open
    popup: Option<WindowId>,
}

/// Global application state shared across all UI components via `std::sync::Arc<std::sync::RwLock<>>`.
pub struct AppState {
    /// Loaded configuration including machine list and metric selections (shared via std::sync::Arc<std::sync::RwLock>).
    pub config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>>,
    /// Currently visible view state - determines which UI panel is displayed
    pub current_view: View,
    /// Settings window for general configuration (always created during init)
    pub settings_window: SettingsWindow,
    /// Remote machines with live metric data (HashMap<machine_name, RemoteMachine>)
    pub machines: std::collections::HashMap<String, crate::remote_machine::RemoteMachine>,
    /// Local machine always present - collected directly via nmd_service::MetricsAggregator
    pub local_machine: crate::remote_machine::RemoteMachine,
    /// Receiver for local metrics packets (from background thread running MetricsAggregator)
    local_metrics_rx: std::sync::Mutex<std::sync::mpsc::Receiver<nmd_service::MetricPacket>>,
    /// Local machine config (127.0.0.1, hostname-based name)
    pub local_machine_config: crate::config::manager::MachineConfig,
    /// PairingManager manages paired machines and their ECDH-derived shared keys
    pub pairing_manager: std::sync::Arc<std::sync::RwLock<crate::pairing_manager::PairingManager>>,
    /// In-memory queue of pending pairing requests waiting for user approval (60-second timeout)
    pub pending_pairings: Vec<crate::pairing_manager::PairingRequest>,
    /// Which machine is being configured in sensor_config views
    pub editing_machine_name: Option<String>,
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
    #[cfg(feature = "dev")]
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

        // ── Local machine setup for debug mode ───────────────────────────────
        let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "local".to_string());
        let local_machine_config =
            crate::config::manager::MachineConfig::new(hostname.clone(), "127.0.0.1".to_string());
        let local_machine = RemoteMachine::new_debug(hostname.clone());
        let (_tx, local_rx) = std::sync::mpsc::channel::<nmd_service::MetricPacket>();
        let local_metrics_rx = std::sync::Mutex::new(local_rx);

        let pairing_manager = Self::create_pairing_manager();

        let mut settings_window = SettingsWindow::new(settings_window_config);
        settings_window.update_config(minimon_config);

        AppState {
            config_manager,
            current_view: View::Panel, // Start at panel view
            settings_window,
            machines,
            local_machine,
            local_metrics_rx,
            local_machine_config,
            pairing_manager,
            pending_pairings: Vec::new(),
            editing_machine_name: None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        use crate::remote_machine::RemoteMachine;

        // Production: ~/.config/cosmic-applet/config.toml
        // Development fallback: ./config.toml (when running from project dir)
        let canonical = crate::config::manager::default_config_path();
        let config = if canonical.exists() {
            log::info!("📂 Loading config from {}", canonical.display());
            ConfigManager::load(canonical.to_str().unwrap_or("config.toml"))
        } else if std::path::Path::new("config.toml").exists() {
            log::info!("📂 Loading config from ./config.toml (development fallback)");
            ConfigManager::load("config.toml")
        } else {
            log::info!("📂 No config found — starting with empty machine list");
            ConfigManager::default()
        };

        let config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>> =
            std::sync::Arc::new(std::sync::RwLock::new(config.clone()));
        let settings_window_config = config_manager.clone();

        // Load saved MinimonConfig from COSMIC config system
        let minimon_config = Self::load_minimon_config();

        // ── Local machine setup ──────────────────────────────────────────────────
        // Determine hostname for the local machine — read directly from the kernel
        let hostname = std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "local".to_string());
        let local_machine_config =
            crate::config::manager::MachineConfig::new(hostname.clone(), "127.0.0.1".to_string());

        // Auto-clean old localhost config entries (migrates old configs)
        {
            let mut cm = config_manager.write().unwrap();
            let had_localhost = cm
                .machines
                .iter()
                .any(|m| m.host == "127.0.0.1" || m.host == "localhost");
            if had_localhost {
                cm.machines
                    .retain(|m| m.host != "127.0.0.1" && m.host != "localhost");
                let _ = cm.save();
                log::info!(
                    "Auto-cleaned localhost machine entries from config — local machine now collected directly"
                );
            }
        }

        // Create RemoteMachine instances from config (skip localhost)
        let config_read = config_manager.read().unwrap();
        let mut machines = std::collections::HashMap::new();
        for machine_config in &config_read.machines {
            // Skip localhost entries — local machine is now always collected directly
            if machine_config.host == "127.0.0.1" || machine_config.host == "localhost" {
                continue;
            }
            machines.insert(
                machine_config.name.clone(),
                RemoteMachine::new(machine_config.name.clone()),
            );
        }
        drop(config_read);

        // Initialize settings_window with config before reading refresh rate for thread
        let mut settings_window = SettingsWindow::new(settings_window_config);
        settings_window.update_config(minimon_config);

        // Read refresh rate from settings_window config before spawning thread
        let local_refresh_ms = settings_window.minimon_config.refresh_rate as u64;

        // Spawn background thread that owns MetricsAggregator — collects local metrics at configured interval
        let (local_tx, local_rx) = std::sync::mpsc::channel::<nmd_service::MetricPacket>();
        let local_metrics_rx = std::sync::Mutex::new(local_rx);
        let hostname_for_thread = hostname.clone();
        std::thread::spawn(move || {
            let mut collector = nmd_service::MetricsAggregator::new(&hostname_for_thread);
            loop {
                let packet = collector.aggregate();
                if local_tx.send(packet).is_err() {
                    break; // Receiver dropped — applet shut down
                }
                std::thread::sleep(std::time::Duration::from_millis(local_refresh_ms));
            }
        });

        // Initialize local_machine with default zero data — first real data arrives within 1s
        let local_machine = crate::remote_machine::RemoteMachine::new(hostname.clone());

        let pairing_manager = Self::create_pairing_manager();

        AppState {
            config_manager,
            current_view: View::Panel, // Default to panel view
            settings_window,
            machines,
            local_machine,
            local_metrics_rx,
            local_machine_config,
            pairing_manager,
            pending_pairings: Vec::new(),
            editing_machine_name: None,
        }
    }
}

/// Entry point — registers the application with Cosmic's panel system.
/// `#[allow(dead_code)]` is required because this file is compiled as both a lib and bin
/// target; the lib compilation sees `main` as unreachable.
#[allow(dead_code)]
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

    #[cfg(feature = "dev")]
    {
        // Check for --test and --debug flags for development/debug modes
        let test_mode = args.contains(&"--test".to_string());
        let debug_mode = args.contains(&"--debug".to_string());

        let config_path = args
            .iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                crate::config::manager::default_config_path()
                    .to_str()
                    .unwrap_or("config.toml")
                    .to_string()
            });

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

    #[cfg(not(feature = "dev"))]
    {
        let config_path = args
            .iter()
            .skip(1)
            .find(|arg| !arg.starts_with("--"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                crate::config::manager::default_config_path()
                    .to_str()
                    .unwrap_or("config.toml")
                    .to_string()
            });

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

    #[cfg(feature = "dev")]
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

            // Spawn TCP pairing listener on same port
            let state_clone_tcp = std::sync::Arc::clone(&shared_state);
            let tcp_port = {
                shared_state
                    .read()
                    .unwrap()
                    .config_manager
                    .read()
                    .unwrap()
                    .udp_port
            };
            std::thread::spawn(move || {
                let rt =
                    tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for TCP");
                rt.block_on(async move {
                    crate::network::tcp_listener::start_tcp_listener(tcp_port, state_clone_tcp)
                        .await;
                });
            });
        } else {
            log::info!("🎲 DEBUG MODE: Skipping UDP/TCP receivers, using fake data");
        }

        // Initialize AppState with shared state (settings_window already created in default).
        (
            PanelApplet {
                core,
                shared_state, // Use the same Arc instance
                popup: None,
            },
            Task::none(),
        )
    }

    #[cfg(not(feature = "dev"))]
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        // Create shared AppState with default values (no debug mode available).
        let app_state = AppState::default();

        // Clone pairing manager before moving app_state into shared_state
        let pairing_manager_clone = app_state.pairing_manager.clone();

        // Wrap in Arc<RwLock> ONCE — both UI and UDP receiver share this instance
        let shared_state = std::sync::Arc::new(std::sync::RwLock::new(app_state));

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

        // Spawn TCP pairing listener on same port
        let state_clone_tcp = std::sync::Arc::clone(&shared_state);
        let tcp_port = {
            shared_state
                .read()
                .unwrap()
                .config_manager
                .read()
                .unwrap()
                .udp_port
        };
        std::thread::spawn(move || {
            let rt =
                tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for TCP");
            rt.block_on(async move {
                crate::network::tcp_listener::start_tcp_listener(tcp_port, state_clone_tcp).await;
            });
        });

        // Initialize AppState with shared state (settings_window already created in default).
        (
            PanelApplet {
                core,
                shared_state, // Use the same Arc instance
                popup: None,
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
                // Drain local metrics channel — update local machine with latest data
                {
                    // Use a separate block to manage locks properly
                    let packets: Vec<nmd_service::MetricPacket> = {
                        let state = self.shared_state.read().unwrap();
                        let rx = state.local_metrics_rx.lock().unwrap();
                        rx.try_iter().collect()
                    };
                    // Update local machine with collected packets (no lock needed)
                    for packet in packets {
                        let mut state = self.shared_state.write().unwrap();
                        state.local_machine.update_from_packet(&packet);
                    }
                }

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
            AppMessage::OpenSettings => {
                // Navigate to machine list from settings.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::MachineList;
                Task::none()
            }
            AppMessage::OpenMachineDetail(machine_name) => {
                // Open machine detail view by name.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = View::MachineDetail(machine_name);
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
            AppMessage::OpenMachineSensorConfig(machine_name) => {
                // Open machine sensor config menu for a specific machine.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.editing_machine_name = Some(machine_name.clone());
                app_state.current_view = View::MachineSensorConfig(machine_name);
                Task::none()
            }
            AppMessage::Back => {
                // Go back to previous view.
                let mut app_state = self.shared_state.write().unwrap();
                app_state.current_view = match &app_state.current_view {
                    View::MachineList => View::Panel,
                    View::MachineDetail(_) => View::MachineList,
                    View::MachineSensorConfig(name) => View::MachineDetail(name.clone()),
                    View::GeneralSettings => View::MachineList,
                    // Sensor config views go back to MachineSensorConfig
                    _ => {
                        if let Some(ref name) = app_state.editing_machine_name.clone() {
                            View::MachineSensorConfig(name.clone())
                        } else {
                            View::MachineList
                        }
                    }
                };
                Task::none()
            }
            AppMessage::RemoveMachine(machine_name) => {
                let mut app_state = self.shared_state.write().unwrap();

                // Remove from live machines map (stops showing in UI immediately)
                app_state.machines.remove(&machine_name);

                // Remove from pairing manager so future packets trigger re-pairing
                {
                    let mut pm = app_state.pairing_manager.write().unwrap();
                    if let Err(e) = pm.remove_pairing(&machine_name) {
                        log::error!("Failed to remove pairing for {}: {}", machine_name, e);
                    }
                }

                // Remove from config and save
                app_state
                    .config_manager
                    .write()
                    .unwrap()
                    .remove_machine(&machine_name);
                let _ = app_state.config_manager.read().unwrap().save();

                // Navigate back: machine list if 2+ remain, otherwise panel
                let machine_count = app_state.machines.len();
                app_state.current_view = if machine_count >= 2 {
                    View::MachineList
                } else {
                    View::Panel
                };
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
                    let machine_id_str = req.machine_id.clone();
                    let host_str = req.host.clone();

                    // Derive ECDH shared key and persist — drop lock before config write
                    let pairing_result = {
                        let mut pm = state.pairing_manager.write().unwrap();
                        pm.add_pairing(req.machine_id.clone(), &req.sender_pubkey, req.host.clone())
                    };

                    match pairing_result {
                        Err(e) => {
                            log::error!("Failed to persist pairing for {}: {}", machine_id_str, e)
                        }
                        Ok(()) => {
                            log::info!("✅ Pairing accepted for machine: {}", machine_id_str);
                            // Persist to config so the machine survives a restart
                            let added = state
                                .config_manager
                                .write()
                                .unwrap()
                                .add_machine(&machine_id_str, &host_str);
                            if added {
                                let _ = state.config_manager.read().unwrap().save();
                            }
                            // Add to live machines map so the UI shows it immediately
                            state
                                .machines
                                .entry(machine_id_str.clone())
                                .or_insert_with(|| {
                                    crate::remote_machine::RemoteMachine::new(
                                        machine_id_str.clone(),
                                    )
                                });
                            // Copy local machine's sensor config as the default for new machines
                            let local_sensor_config =
                                state.settings_window.minimon_config.sensor_config.clone();
                            let mut cm = state.config_manager.write().unwrap();
                            if let Some(mc) =
                                cm.machines.iter_mut().find(|m| m.name == machine_id_str)
                            {
                                mc.sensor_config = local_sensor_config;
                            }
                            let _ = cm.save();
                            // Send TCP response if this request came via TCP
                            if let Some(arc_tx) = req.tcp_response.as_ref() {
                                if let Some(tx) = arc_tx.lock().unwrap().take() {
                                    let receiver_pubkey = state
                                        .pairing_manager
                                        .read()
                                        .unwrap()
                                        .get_receiver_x25519_pubkey();
                                    let _ = tx.send(
                                        crate::pairing_manager::TcpPairingResponse::Accept(
                                            receiver_pubkey,
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
                Task::none()
            }
            AppMessage::DenyPairing(machine_id) => {
                let mut state = self.shared_state.write().unwrap();
                // Extract the request to get tcp_response before dropping
                if let Some(pos) = state
                    .pending_pairings
                    .iter()
                    .position(|r| r.machine_id == machine_id)
                {
                    let req = state.pending_pairings.remove(pos);
                    if let Some(arc_tx) = req.tcp_response.as_ref() {
                        if let Some(tx) = arc_tx.lock().unwrap().take() {
                            let _ = tx.send(crate::pairing_manager::TcpPairingResponse::Deny);
                        }
                    }
                }
                log::info!("❌ Pairing denied for machine: {}", machine_id);
                Task::none()
            }
            AppMessage::Settings(settings_message) => {
                // Forward settings window message to settings_window handler.
                let mut app_state = self.shared_state.write().unwrap();

                match settings_message {
                    crate::ui::settings_window::SettingsMessage::CloseWindow => {
                        app_state.current_view = View::MachineList;
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

                    crate::ui::settings_window::SettingsMessage::NoOp => {
                        // No operation — do nothing.
                    }
                }

                Task::none()
            }

            // CPU sensor configuration toggles
            AppMessage::ToggleCpuShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cpu
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cpu.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleCpuShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cpu
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cpu.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleCpuShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cpu
                        .show_label(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cpu.show_label(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleCpuShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cpu
                        .show_icon(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cpu.show_icon(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }

            // CPU Temperature sensor configuration toggles
            AppMessage::ToggleCpuTempShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cputemp
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cputemp.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleCpuTempShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cputemp
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cputemp.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleCpuTempShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cputemp
                        .show_label(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cputemp.show_label(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleCpuTempShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .cputemp
                        .show_icon(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.cputemp.show_icon(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }

            // Memory sensor configuration toggles
            AppMessage::ToggleMemoryShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .memory
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.memory.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleMemoryShowAllocated(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .memory
                        .show_allocated = enabled;
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.memory.show_allocated = enabled;
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleMemoryShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .memory
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.memory.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleMemoryShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .memory
                        .show_label(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.memory.show_label(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleMemoryShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .memory
                        .show_icon(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.memory.show_icon(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleMemoryAsPercentage(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .memory
                        .percentage = enabled;
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.memory.percentage = enabled;
                    }
                    let _ = cm.save();
                }
                Task::none()
            }

            // Network sensor configuration toggles
            AppMessage::ToggleNetworkCombine(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    if enabled {
                        state
                            .settings_window
                            .minimon_config
                            .sensor_config
                            .network1
                            .variant = crate::minimon_config::NetworkVariant::Combined;
                    } else {
                        state
                            .settings_window
                            .minimon_config
                            .sensor_config
                            .network1
                            .variant = crate::minimon_config::NetworkVariant::Download;
                    }
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        if enabled {
                            mc.sensor_config.network1.variant =
                                crate::minimon_config::NetworkVariant::Combined;
                        } else {
                            mc.sensor_config.network1.variant =
                                crate::minimon_config::NetworkVariant::Download;
                        }
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleNetworkShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .network1
                        .show_label(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.network1.show_label(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleNetworkShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .network1
                        .show_icon(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.network1.show_icon(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleNetworkShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .network1
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.network1.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleNetworkShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .network1
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.network1.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }

            // Disk sensor configuration toggles
            AppMessage::ToggleDiskCombine(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    if enabled {
                        state
                            .settings_window
                            .minimon_config
                            .sensor_config
                            .disks1
                            .variant = crate::minimon_config::DisksVariant::Combined;
                    } else {
                        state
                            .settings_window
                            .minimon_config
                            .sensor_config
                            .disks1
                            .variant = crate::minimon_config::DisksVariant::Write;
                    }
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        if enabled {
                            mc.sensor_config.disks1.variant =
                                crate::minimon_config::DisksVariant::Combined;
                        } else {
                            mc.sensor_config.disks1.variant =
                                crate::minimon_config::DisksVariant::Write;
                        }
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleDiskShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .disks1
                        .show_label(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.disks1.show_label(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleDiskShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .disks1
                        .show_icon(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.disks1.show_icon(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleDiskWriteShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .disks1
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.disks1.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleDiskWriteShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .disks1
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.disks1.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleDiskReadShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .disks2
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.disks2.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleDiskReadShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .disks2
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.disks2.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }

            // GPU sensor configuration toggles
            AppMessage::ToggleGpuShowLabel(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .usage
                        .show_label(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.usage.show_label(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuShowIcon(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .usage
                        .show_icon(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.usage.show_icon(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuLoadShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .usage
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.usage.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuLoadShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .usage
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.usage.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuVramShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .vram
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.vram.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuVramAsPercentage(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .vram
                        .percentage = enabled;
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.vram.percentage = enabled;
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuTempShowChart(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .temp
                        .show_chart(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.temp.show_chart(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::ToggleGpuTempShowValue(enabled) => {
                let mut state = self.shared_state.write().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                if editing.as_deref() == Some(&local_name) || editing.is_none() {
                    state
                        .settings_window
                        .minimon_config
                        .sensor_config
                        .gpu
                        .temp
                        .show_value(enabled);
                    AppState::save_minimon_config(&state.settings_window.minimon_config);
                } else if let Some(machine_name) = editing {
                    let mut cm = state.config_manager.write().unwrap();
                    if let Some(mc) = cm.machines.iter_mut().find(|m| m.name == machine_name) {
                        mc.sensor_config.gpu.temp.show_value(enabled);
                    }
                    let _ = cm.save();
                }
                Task::none()
            }
            AppMessage::CopyToClipboard(text) => {
                return cosmic::iced::clipboard::write(text);
            }
            AppMessage::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(
                        p,
                    ));
                } else {
                    return cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                        |_| Default::default(),
                        |app: &mut PanelApplet| {
                            let new_id = WindowId::unique();
                            app.popup.replace(new_id);
                            let mut popup_settings = app.core.applet.get_popup_settings(
                                app.core.main_window_id().unwrap(),
                                new_id,
                                Some((1, 1)),
                                None,
                                None,
                            );
                            popup_settings.positioner.size_limits = cosmic::iced::Limits::NONE
                                .min_width(360.0)
                                .max_width(460.0)
                                .min_height(100.0)
                                .max_height(700.0);
                            popup_settings
                        },
                        None,
                    ));
                }
            }
            AppMessage::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        // Determine if panel is horizontal (top/bottom) vs vertical (left/right)
        let is_horizontal = matches!(
            self.core.applet.anchor,
            cosmic::applet::cosmic_panel_config::PanelAnchor::Top
                | cosmic::applet::cosmic_panel_config::PanelAnchor::Bottom
        );

        let mut limits = Limits::NONE.min_width(1.0).min_height(1.0);
        if let Some(b) = self.core.applet.suggested_bounds {
            if b.width > 0.0 {
                limits = limits.max_width(b.width);
            }
            if b.height > 0.0 {
                limits = limits.max_height(b.height);
            }
        }

        // Always use local_machine — it's always present and always up-to-date
        let (local_machine, pending_count, minimon_config) = {
            let state = self.shared_state.read().unwrap();
            let local_machine = state.local_machine.clone();
            let pending_count = state.pending_pairings.len();
            let minimon_config = state.settings_window.minimon_config.clone();
            (local_machine, pending_count, minimon_config)
        };

        // Panel always shows local machine sensor readings
        let display = crate::ui::panel_widget::GlobalDisplayConfig::from_minimon(&minimon_config);
        let inner: Element<'_, Self::Message> =
            crate::ui::panel_widget::PanelWidget::view_from_machines(
                &[local_machine],
                &minimon_config.content_order,
                &minimon_config.sensor_config, // local machine's sensor config
                &display,
            );

        // Add pending pairing badge if needed
        let panel_content: Element<'_, Self::Message> = if pending_count > 0 {
            cosmic::widget::row(vec![inner, crate::pairing_ui::pending_badge(pending_count)])
                .align_y(cosmic::iced::Alignment::Center)
                .into()
        } else {
            inner
        };

        // Wrap in a button that opens the popup on click
        let button = cosmic::widget::button::custom(panel_content)
            .padding(if is_horizontal {
                [0, self.core.applet.suggested_padding(true).1]
            } else {
                [self.core.applet.suggested_padding(true).0, 0]
            })
            .class(cosmic::theme::Button::AppletIcon)
            .on_press(AppMessage::TogglePopup);

        autosize::autosize(cosmic::widget::container(button), AUTOSIZE_MAIN_ID.clone())
            .limits(limits)
            .into()
    }

    fn view_window(&self, _id: WindowId) -> Element<'_, Self::Message> {
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

        // Pairing UI hijack: pending pairings always take over the popup view.
        if pending_count > 0 {
            return self
                .core
                .applet
                .popup_container(crate::pairing_ui::view(&pending_pairings_clone))
                .limits(
                    Limits::NONE
                        .max_width(460.0)
                        .min_width(360.0)
                        .min_height(100.0)
                        .max_height(700.0),
                )
                .into();
        }

        // Render based on current UI state — SettingsWindow overlays other views.
        if settings_visible {
            // Show SettingsWindow when visible (overlay mode).
            let state = self.shared_state.read().unwrap();
            let minimon_config = state.settings_window.minimon_config.clone();
            drop(state);

            return self
                .core
                .applet
                .popup_container(
                    crate::ui::settings_window::view_with_config(&minimon_config)
                        .map(|msg| AppMessage::Settings(msg)),
                )
                .limits(
                    Limits::NONE
                        .max_width(460.0)
                        .min_width(360.0)
                        .min_height(100.0)
                        .max_height(700.0),
                )
                .into();
        }

        // Render view based on current_view state
        let content: Element<'_, Self::Message> = match current_view {
            View::Panel | View::MachineList => {
                let state = self.shared_state.read().unwrap();
                let local_machine = state.local_machine.clone();
                let local_machine_name = state.local_machine.name.clone();
                let remote_machines: Vec<_> = state.machines.values().cloned().collect();
                let content_order = state.settings_window.minimon_config.content_order.clone();
                let minimon_config = state.settings_window.minimon_config.clone();
                drop(state);

                // Build machines list with their sensor configs
                let mut all_machines = vec![local_machine];
                all_machines.extend(remote_machines);

                crate::ui::machine_list::view(
                    &all_machines,
                    &content_order,
                    &local_machine_name,
                    &minimon_config.sensor_config, // for local machine
                    &config,
                    &minimon_config,
                )
            }
            View::MachineSensorConfig(ref machine_name) => {
                let state = self.shared_state.read().unwrap();
                let local_machine = state.local_machine.clone();

                if local_machine.name == *machine_name {
                    let sensor_config = state.settings_window.minimon_config.sensor_config.clone();
                    drop(state);
                    crate::ui::machine_sensor_config_menu::view(
                        machine_name,
                        Some(&local_machine),
                        &sensor_config,
                    )
                } else {
                    let machine_opt = state.machines.get(machine_name).cloned();
                    let sensor_config = config
                        .machines
                        .iter()
                        .find(|m| m.name == *machine_name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default();
                    drop(state);
                    crate::ui::machine_sensor_config_menu::view(
                        machine_name,
                        machine_opt.as_ref(),
                        &sensor_config,
                    )
                }
            }
            View::MachineDetail(ref machine_name) => {
                let state = self.shared_state.read().unwrap();
                let local_machine = state.local_machine.clone();
                let local_machine_config = state.local_machine_config.clone();
                let minimon_config = state.settings_window.minimon_config.clone();

                if local_machine.name == *machine_name {
                    // Local machine detail — no Remove button
                    // Use saved sensor config from settings window
                    let mut lmc = local_machine_config.clone();
                    lmc.sensor_config = minimon_config.sensor_config.clone();
                    drop(state);
                    crate::ui::machine_detail::view(&lmc, &local_machine, &minimon_config, true)
                } else if let Some(remote_machine) = state.machines.get(machine_name).cloned() {
                    let config_entry = config
                        .machines
                        .iter()
                        .find(|m| m.name == *machine_name)
                        .cloned();
                    drop(state);
                    if let Some(config_entry) = config_entry {
                        crate::ui::machine_detail::view(
                            &config_entry,
                            &remote_machine,
                            &minimon_config,
                            false,
                        )
                    } else {
                        cosmic::widget::button::text("← Back")
                            .on_press(AppMessage::Back)
                            .into()
                    }
                } else {
                    drop(state);
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
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                let sensor_config = if editing.as_deref() == Some(&local_name) || editing.is_none()
                {
                    state.settings_window.minimon_config.sensor_config.clone()
                } else if let Some(ref name) = editing {
                    config
                        .machines
                        .iter()
                        .find(|m| &m.name == name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default()
                } else {
                    state.settings_window.minimon_config.sensor_config.clone()
                };
                drop(state);
                crate::ui::sensor_config::view_cpu_config(&sensor_config)
            }
            View::CpuTempConfig => {
                let state = self.shared_state.read().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                let sensor_config = if editing.as_deref() == Some(&local_name) || editing.is_none()
                {
                    state.settings_window.minimon_config.sensor_config.clone()
                } else if let Some(ref name) = editing {
                    config
                        .machines
                        .iter()
                        .find(|m| &m.name == name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default()
                } else {
                    state.settings_window.minimon_config.sensor_config.clone()
                };
                drop(state);
                crate::ui::sensor_config::view_cpu_temp_config(&sensor_config)
            }
            View::MemoryConfig => {
                let state = self.shared_state.read().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                let sensor_config = if editing.as_deref() == Some(&local_name) || editing.is_none()
                {
                    state.settings_window.minimon_config.sensor_config.clone()
                } else if let Some(ref name) = editing {
                    config
                        .machines
                        .iter()
                        .find(|m| &m.name == name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default()
                } else {
                    state.settings_window.minimon_config.sensor_config.clone()
                };
                drop(state);
                crate::ui::sensor_config::view_memory_config(&sensor_config)
            }
            View::NetworkConfig => {
                let state = self.shared_state.read().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                let sensor_config = if editing.as_deref() == Some(&local_name) || editing.is_none()
                {
                    state.settings_window.minimon_config.sensor_config.clone()
                } else if let Some(ref name) = editing {
                    config
                        .machines
                        .iter()
                        .find(|m| &m.name == name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default()
                } else {
                    state.settings_window.minimon_config.sensor_config.clone()
                };
                drop(state);
                crate::ui::sensor_config::view_network_config(&sensor_config)
            }
            View::DiskConfig => {
                let state = self.shared_state.read().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                let sensor_config = if editing.as_deref() == Some(&local_name) || editing.is_none()
                {
                    state.settings_window.minimon_config.sensor_config.clone()
                } else if let Some(ref name) = editing {
                    config
                        .machines
                        .iter()
                        .find(|m| &m.name == name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default()
                } else {
                    state.settings_window.minimon_config.sensor_config.clone()
                };
                drop(state);
                crate::ui::sensor_config::view_disk_config(&sensor_config)
            }
            View::GpuConfig => {
                let state = self.shared_state.read().unwrap();
                let editing = state.editing_machine_name.clone();
                let local_name = state.local_machine.name.clone();
                let sensor_config = if editing.as_deref() == Some(&local_name) || editing.is_none()
                {
                    state.settings_window.minimon_config.sensor_config.clone()
                } else if let Some(ref name) = editing {
                    config
                        .machines
                        .iter()
                        .find(|m| &m.name == name)
                        .map(|m| m.sensor_config.clone())
                        .unwrap_or_default()
                } else {
                    state.settings_window.minimon_config.sensor_config.clone()
                };
                drop(state);
                crate::ui::sensor_config::view_gpu_config(&sensor_config)
            }
        };

        self.core
            .applet
            .popup_container(content)
            .limits(
                Limits::NONE
                    .max_width(460.0)
                    .min_width(360.0)
                    .min_height(100.0)
                    .max_height(700.0),
            )
            .into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn on_close_requested(&self, id: WindowId) -> Option<Self::Message> {
        Some(AppMessage::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        // Refresh at 1Hz — matches the nmd-service send rate so there's no point
        // re-rendering faster than data actually arrives. Reducing from 500ms to 1000ms
        // halves the number of expensive view() re-renders (canvas ring charts) during scroll.
        cosmic::iced::time::every(std::time::Duration::from_millis(1000))
            .map(|_| AppMessage::RefreshMetrics)
    }
}
