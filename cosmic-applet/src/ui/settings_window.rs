//! # SettingsWindow — Configuration menu for adding/removing machines and toggling metrics
//!
//! Provides a settings window that allows users to:
//! - Add new machines (with name, host/IP, port)
//! - Remove existing machines
//! - Enable/disable machines (checkbox)
//! - Toggle which metrics to display per machine: CPU, memory, disk, network, uptime, GPU VRAM, temperature
//!
//! Configuration is persisted via the shared ConfigManager (Arc<RwLock>) using TOML format.

use crate::config::manager::ConfigManager;
use cosmic::iced::widget as iced_widget;

/// Message types for settings window.
#[derive(Debug, Clone)]
pub enum SettingsMessage {
    /// Add a new machine to the configuration.
    AddMachine,
    /// Remove the selected machine from the configuration.
    RemoveSelected,
    /// Close the settings window.
    CloseWindow,
    /// No operation — used when a widget needs to return a message but no action is required.
    NoOp,
    /// Update a field in a specific machine configuration.
    UpdateMachineField(usize, MachineField, String),
    /// Toggle a metric display setting for a specific machine.
    UpdateMachineMetric(usize, MetricType, bool),
}

/// Field types that can be updated in a machine configuration.
#[derive(Debug, Clone)]
pub enum MachineField {
    Name,
    Host,
    Port,
    Enabled,
}

/// Metric types that can be toggled per machine.
#[derive(Debug, Clone)]
pub enum MetricType {
    CPU,
    Memory,
    Disk,
    Network,
    Uptime,
    GpuVram,
    Temperature,
}

/// SettingsWindow renders a configuration menu for managing machines and metric selections.
///
/// Layout: list of machines with editable fields + enable toggle + metric checkboxes,
/// plus add/remove buttons at the bottom. All changes update shared ConfigManager
/// and persist to disk via ConfigManager::save().
#[derive(Clone)]
pub struct SettingsWindow {
    /// Shared configuration manager (std::sync::Arc<RwLock>) for reading/writing machine configs.
    pub config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>>,
    /// Whether this window is currently visible (toggled from panel widget).
    pub visible: bool,
}

impl SettingsWindow {
    /// Create a new SettingsWindow with the given shared configuration manager.
    ///
    /// The window starts invisible — it's toggled on via a gear icon button in the panel widget.
    pub fn new(config_manager: std::sync::Arc<std::sync::RwLock<ConfigManager>>) -> Self {
        SettingsWindow {
            config_manager,
            visible: false,
        }
    }

    /// Toggle visibility of the settings window.
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    /// Render the settings window as a Cosmic widget element (if visible).
    ///
    /// Layout:
    /// - Header row with "Settings" title and close button
    /// - Machine list: each machine has name/host/port text inputs, enable toggle, metric checkboxes
    /// - Add machine button + remove selected button at bottom
    pub fn view(&self) -> cosmic::Element<'static, SettingsMessage> {
        if !self.visible {
            return cosmic::widget::text("").into();
        }

        // Read config and clone machines to avoid borrowing issues.
        let config = self.config_manager.read().unwrap().clone();
        
        // Clone machines into a Vec for rendering (each machine is cloned once).
        let machines_clone: Vec<crate::config::manager::MachineConfig> = config.machines.iter().cloned().collect();

        // Machine list with editable fields per entry - clone machines to avoid borrowing issues.
        let bottom_controls = cosmic::widget::Row::new()
            .spacing(8)
            .padding([16, 0])
            .push(
                cosmic::iced::widget::button("Add Machine")
                    .on_press(SettingsMessage::AddMachine)
            )
            .push(
                cosmic::iced::widget::button("Remove Selected")
                    .on_press(SettingsMessage::RemoveSelected)
            );

        // Metric configuration section with ring chart toggles (minimon pattern).
        let metric_config_section = Self::render_metric_config_section();

        // Full settings window layout.
        let content = cosmic::widget::Column::new()
            .spacing(8)
            .padding([16, 24])
            .push(
                cosmic::widget::Row::new()
                    .spacing(8)
                    .push(cosmic::widget::text("Settings").size(24))
                    .push(
                        cosmic::iced::widget::button("×")
                            .on_press(SettingsMessage::CloseWindow)
                    )
            )
            .push(metric_config_section)
            .push(cosmic::widget::Container::new(
                cosmic::widget::Column::with_children(
                    machines_clone.iter()
                        .enumerate()
                        .map(|(idx, machine)| Self::render_machine_row(idx, machine.clone()))
                )
                    .spacing(12)
            ))
            .push(bottom_controls);

        cosmic::widget::container(content)
            .width(700)
            .height(500)
            .into()
    }

    /// Render metric configuration section with ring chart toggles (minimon pattern).
    ///
    /// Uses standard cosmic widgets for togglers since cosmic::widget::settings may not be available.
    fn render_metric_config_section() -> cosmic::Element<'static, SettingsMessage> {
        // Metric display settings using standard cosmic widgets
        let content = cosmic::widget::Column::new()
            .spacing(8)
            .padding([16, 0])
            .push(cosmic::widget::text("Metric Display").size(24))
            .push(
                cosmic::widget::Container::new(
                    cosmic::widget::Row::new()
                        .spacing(16)
                        .push(
                            cosmic::widget::Column::new()
                                .spacing(8)
                                .push(cosmic::widget::text("Ring charts"))
                                .push(
                                    iced_widget::toggler(false)
                                        .on_toggle(|_| SettingsMessage::NoOp) // Placeholder
                                )
                        )
                        .push(
                            cosmic::widget::Column::new()
                                .spacing(8)
                                .push(cosmic::widget::text("Show values"))
                                .push(
                                    iced_widget::toggler(false)
                                        .on_toggle(|_| SettingsMessage::NoOp) // Placeholder
                                )
                        )
                )
            );
        
        cosmic::widget::container(content).padding(8).into()
    }

    /// Render one row in the machine list with editable fields.
    fn render_machine_row(index: usize, machine: crate::config::manager::MachineConfig) -> cosmic::Element<'static, SettingsMessage> {
        // Clone machine data into closures to avoid borrow issues
        let machine_name = machine.name.clone();
        let machine_host = machine.host.clone();
        let machine_port = machine.port;
        let machine_enabled = machine.enabled;
        let machine_show_cpu = machine.show_cpu;
        let machine_show_memory = machine.show_memory;
        let machine_show_disk = machine.show_disk;
        let machine_show_network = machine.show_network;
        let machine_show_uptime = machine.show_uptime;
        let machine_show_gpu_vram = machine.show_gpu_vram;
        let machine_show_temperature = machine.show_temperature;

        // Text inputs for name, host, port (use cloned String directly)
        let name_input = cosmic::widget::TextInput::new("Name", machine_name.clone())
            .on_input(move |value| SettingsMessage::UpdateMachineField(index, MachineField::Name, value))
            .width(150);

        let host_input = cosmic::widget::TextInput::new("Host", machine_host.clone())
            .on_input(move |value| SettingsMessage::UpdateMachineField(index, MachineField::Host, value))
            .width(200);

        let port_input = cosmic::widget::TextInput::new("Port", machine_port.to_string())
            .on_input(move |value| {
                if let Ok(port) = value.parse::<u16>() {
                    SettingsMessage::UpdateMachineField(index, MachineField::Port, port.to_string())
                } else {
                    SettingsMessage::NoOp
                }
            })
            .width(80);

        // Enable/disable toggle.
        let enable_toggle = iced_widget::checkbox(machine_enabled)
            .on_toggle(move |enabled| SettingsMessage::UpdateMachineField(index, MachineField::Enabled, enabled.to_string()));

        // Metric checkboxes.
        let metric_row = cosmic::widget::Row::new()
            .spacing(8)
            .push(
                cosmic::widget::text("CPU").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_cpu)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::CPU, checked))
            )
            .push(
                cosmic::widget::text("MEM").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_memory)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::Memory, checked))
            )
            .push(
                cosmic::widget::text("DISK").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_disk)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::Disk, checked))
            )
            .push(
                cosmic::widget::text("NET").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_network)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::Network, checked))
            )
            .push(
                cosmic::widget::text("UP").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_uptime)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::Uptime, checked))
            )
            .push(
                cosmic::widget::text("GPU").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_gpu_vram)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::GpuVram, checked))
            )
            .push(
                cosmic::widget::text("TMP").size(12).width(40)
                    .align_y(cosmic::iced::Alignment::Center)
            )
            .push(
                iced_widget::checkbox(machine_show_temperature)
                    .on_toggle(move |checked| SettingsMessage::UpdateMachineMetric(index, MetricType::Temperature, checked))
            );

        // Machine row container with all fields.
        cosmic::widget::Container::new(
            cosmic::widget::Column::new()
                .spacing(8)
                .push(
                    cosmic::widget::Row::new()
                        .spacing(8)
                        .push(name_input)
                        .push(host_input)
                        .push(port_input)
                        .push(cosmic::widget::Text::new("Enable").size(12))
                        .push(enable_toggle)
                )
                .push(metric_row)
        )
        .padding([8, 0])
        .into()
    }

    /// Close the settings window (set visible = false).
    pub fn close(&mut self) {
        self.visible = false;
    }
}
