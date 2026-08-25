//! # GridWindow — Click-to-expand window showing all remote machines in a grid layout
//!
//! When the user clicks the panel widget, this window opens displaying every registered machine
//! as a row with columns for each metric (CPU/memory/disk/network/uptime/GPU/VRAM/temp).
//! Status indicators show ● (online) or ○ (offline/pending). Color-coded progress bars
//! apply 60%/80% thresholds. Updates in real-time as UDP packets arrive from remote machines.

use crate::ui::{MachineRow, MachineStatus};
use crate::AppMessage;
use cosmic::iced::widget::{container, text};

/// GridWindow renders all remote machines in a grid layout with per-metric columns.
///
/// Layout: one row per machine, columns for name + each metric type. Status indicators (●/○)
/// appear next to the machine name. Progress bars are color-coded at 60%/80% thresholds.
#[derive(Clone)]
pub struct GridWindow {
    /// All registered machines with their current metrics and status — updated by UDP receiver.
    pub rows: Vec<MachineRow>,
    /// Whether this window is currently visible (toggled on panel click).
    pub visible: bool,
}

impl GridWindow {
    /// Create a new empty GridWindow — no machines shown until UDP data arrives or config loads.
    ///
    /// The grid starts with header row only and populates rows as MetricPacket data is received.
    pub fn new() -> Self {
        GridWindow {
            rows: Vec::new(),
            visible: false,
        }
    }

    /// Create a GridWindow initialized with metrics from an incoming packet.
    /// Creates one row for the machine in the packet with Online status.
    ///
    /// # Arguments
    ///
    /// * `packet` - MetricPacket containing the initial machine metrics
    pub fn new_with_metrics(packet: &nmd_service::packet::MetricPacket) -> Self {
        let mut grid = GridWindow::new();
        
        // Convert machine_id [u8; 20] to string (null-padded)
        let len = packet.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
        let machine_name = std::str::from_utf8(&packet.machine_id[..len])
            .unwrap_or("<unknown>")
            .to_string();
        
        // Create a row from the packet data
        let row = MachineRow {
            name: machine_name,
            status: MachineStatus::Online,
            cpu_usage: Some(packet.cpu_usage),
            memory_usage: Some(packet.memory_used_percent),
            disk_usage: Some(packet.disk_used_percent),
            network_rx_bytes: packet.network_rx_bytes,
            network_tx_bytes: 0,  // TX not available in current packet
            uptime_seconds: packet.uptime_seconds,
            gpu_vram_usage: packet.gpu_vram_used_mb.map(|v| (v as f32 / 8192.0) * 100.0),
            temperature_celsius: packet.temperature_celsius,
        };
        
        grid.rows.push(row);
        grid
    }

    /// Populate the grid with machines from config (called on applet startup).
    /// All machines start in "Pending" status until first UDP packet arrives.
    pub fn populate_from_config(&mut self, machine_names: &[String]) {
        self.rows.clear();
        for name in machine_names {
            let row = MachineRow::new(name.clone(), MachineStatus::Pending);
            self.rows.push(row);
        }
    }

    /// Render the grid window as a Cosmic widget element.
    ///
    /// Columns: Name | CPU% | MEM% | DISK% | NET(rx/tx) | UPTIME | GPU(VRAM) | TEMP(°C) | Status
    /// Each percentage metric has a color-coded progress bar at 60%/80% thresholds.
    pub fn view(&self) -> cosmic::Element<'_, AppMessage> {
        // Header row with column titles for each metric type.
        let header_row: cosmic::Element<'_, AppMessage> = cosmic::iced::widget::Row::new()
            .push(cosmic::iced::widget::Text::new("Machine"))
            .push(cosmic::iced::widget::Text::new("CPU"))
            .push(cosmic::iced::widget::Text::new("MEM"))
            .push(cosmic::iced::widget::Text::new("DISK"))
            .push(cosmic::iced::widget::Text::new("NET(rx/tx)"))
            .push(cosmic::iced::widget::Text::new("UPTIME"))
            .push(cosmic::iced::widget::Text::new("GPU(VRAM)"))
            .push(cosmic::iced::widget::Text::new("TEMP(°C)"))
            .into();

        // One row per machine with metric progress bars and status indicators.
        let mut rows: Vec<cosmic::Element<'_, AppMessage>> = self.rows.iter().map(|row| {
            let header_text: cosmic::Element<'_, AppMessage> = cosmic::widget::text(format!("{} {}", row.status.symbol(), row.name)).into();
            
            // CPU usage progress bar
            let cpu_bar: cosmic::Element<'static, AppMessage> = row.cpu_usage.map(|cpu| {
                container(text(format!("{:.0}%", cpu)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("--%").into()
            });
            
            // Memory usage progress bar
            let mem_bar: cosmic::Element<'static, AppMessage> = row.memory_usage.map(|mem| {
                container(text(format!("{:.0}%", mem)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("--%").into()
            });
            
            // Disk usage progress bar
            let disk_bar: cosmic::Element<'static, AppMessage> = row.disk_usage.map(|disk| {
                container(text(format!("{:.0}%", disk)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("--%").into()
            });
            
            let net_text: cosmic::Element<'_, AppMessage> = cosmic::widget::text(format!(
                "↓{} ↑N/A",
                crate::utils::formatting::format_network_rate(row.network_rx_bytes)
            )).into();
            let uptime_text: cosmic::Element<'_, AppMessage> = cosmic::widget::text(crate::utils::formatting::format_uptime(row.uptime_seconds)).into();
            
            // GPU VRAM progress bar (if available)
            let gpu_bar_or_text: cosmic::Element<'_, AppMessage> = row.gpu_vram_usage.map(|vram| {
                container(text(format!("{:.0}%", vram)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("GPU: --%").into()
            });
            
            // Temperature progress bar (if available)
            let temp_bar_or_text: cosmic::Element<'_, AppMessage> = row.temperature_celsius.map(|temp| {
                container(text(format!("{:.1}°C", temp)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("TEMP: --°C").into()
            });

            let row_widget = cosmic::iced::widget::Row::new()
                .push(header_text)
                .push(cpu_bar)
                .push(mem_bar)
                .push(disk_bar)
                .push(net_text)
                .push(uptime_text)
                .push(gpu_bar_or_text)
                .push(temp_bar_or_text);
            
            row_widget.into()
        }).collect();

        rows.insert(0, header_row.into());

        // Wrap in a scrollable container for grid window.
        cosmic::iced::widget::Column::with_children(rows)
            .spacing(4)
            .padding(16)
            .into()
    }

    /// Render from cloned row data without borrowing GridWindow state.
    pub fn view_with_data(rows: &[MachineRow]) -> cosmic::Element<'static, AppMessage> {
        // Header row with column titles for each metric type.
        let header_row: cosmic::Element<'static, AppMessage> = cosmic::iced::widget::Row::new()
            .push(cosmic::iced::widget::Text::new("Machine"))
            .push(cosmic::iced::widget::Text::new("CPU"))
            .push(cosmic::iced::widget::Text::new("MEM"))
            .push(cosmic::iced::widget::Text::new("DISK"))
            .push(cosmic::iced::widget::Text::new("NET(rx/tx)"))
            .push(cosmic::iced::widget::Text::new("UPTIME"))
            .push(cosmic::iced::widget::Text::new("GPU(VRAM)"))
            .push(cosmic::iced::widget::Text::new("TEMP(°C)"))
            .into();

        // One row per machine with metric progress bars and status indicators.
        let mut rows_elements: Vec<cosmic::Element<'static, AppMessage>> = rows.iter().map(|row| {
            let header_text: cosmic::Element<'static, AppMessage> = cosmic::widget::text(format!("{} {}", row.status.symbol(), row.name)).into();
            
            // CPU usage progress bar
            let cpu_bar: cosmic::Element<'static, AppMessage> = row.cpu_usage.map(|cpu| {
                container(text(format!("{:.0}%", cpu)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("--%").into()
            });
            
            // Memory usage progress bar
            let mem_bar: cosmic::Element<'static, AppMessage> = row.memory_usage.map(|mem| {
                container(text(format!("{:.0}%", mem)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("--%").into()
            });
            
            // Disk usage progress bar
            let disk_bar: cosmic::Element<'static, AppMessage> = row.disk_usage.map(|disk| {
                container(text(format!("{:.0}%", disk)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("--%").into()
            });
            
            let net_text: cosmic::Element<'static, AppMessage> = cosmic::widget::text(format!(
                "↓{} ↑N/A",
                crate::utils::formatting::format_network_rate(row.network_rx_bytes)
            )).into();
            let uptime_text: cosmic::Element<'static, AppMessage> = cosmic::widget::text(crate::utils::formatting::format_uptime(row.uptime_seconds)).into();
            
            // GPU VRAM progress bar (if available)
            let gpu_bar_or_text: cosmic::Element<'static, AppMessage> = row.gpu_vram_usage.map(|vram| {
                container(text(format!("{:.0}%", vram)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("GPU: --%").into()
            });
            
            // Temperature progress bar (if available)
            let temp_bar_or_text: cosmic::Element<'static, AppMessage> = row.temperature_celsius.map(|temp| {
                container(text(format!("{:.1}°C", temp)))
                    .width(cosmic::iced::Length::Fill)
                    .height(cosmic::iced::Length::Fixed(32.0))
                    .into()
            }).unwrap_or_else(|| {
                text("TEMP: --°C").into()
            });

            let row_widget = cosmic::iced::widget::Row::new()
                .push(header_text)
                .push(cpu_bar)
                .push(mem_bar)
                .push(disk_bar)
                .push(net_text)
                .push(uptime_text)
                .push(gpu_bar_or_text)
                .push(temp_bar_or_text);
            
            row_widget.into()
        }).collect();

        rows_elements.insert(0, header_row.into());

        // Wrap in a scrollable container for grid window.
        cosmic::iced::widget::Column::with_children(rows_elements)
            .spacing(4)
            .padding(16)
            .into()
    }

    /// Toggle window visibility (called by PanelWidget click handler).
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    /// Update a machine's metrics from an incoming rkyv-encoded MetricPacket, updating status to Online.
    /// Called by UdpReceiver when new data arrives for a registered machine_id. Finds the matching
    /// MachineRow (by name) and delegates field updates via `MachineRow::update_from_archived`.
    ///
    /// This method performs zero-copy parsing internally — it does NOT deserialize into an owned type.
    /// Prefer calling [`GridWindow::update_machine_metrics_from_archived`] directly when you already
    /// hold an `&ArchivedMetricPacket` reference, to avoid the redundant parse step.
    #[deprecated(
        note = "Use update_machine_metrics_from_archived() with a pre-parsed &ArchivedMetricPacket for zero-copy efficiency",
        since = "0.2.0"
    )]
    pub fn update_machine_metrics(&mut self, machine_id: &str, packet_data: &[u8]) {
        // Parse the raw bytes into an ArchivedMetricPacket reference (zero-copy — no allocation).
        match rkyv::access::<nmd_service::ArchivedMetricPacket, rkyv::rancor::Error>(packet_data) {
            Ok(archived) => self.update_machine_metrics_from_archived(archived),
            Err(e) => log::warn!("Failed to zero-copy parse packet for machine '{}': {}", machine_id, e),
        }
    }

    /// Update a machine's metrics from a zero-copy ArchivedMetricPacket reference — no deserialization.
    /// Called by UdpReceiver when new data arrives for a registered machine_id. Finds the matching
    /// MachineRow (by name) and delegates field updates via `MachineRow::update_from_archived`.
    pub fn update_machine_metrics_from_archived(&mut self, archived: &nmd_service::ArchivedMetricPacket) {
        // machine_id is [u8; 20] — convert to string (null-padded encoding).
        let len = archived.machine_id.iter().position(|&b| b == 0).unwrap_or(20);
        let machine_id: &str = std::str::from_utf8(&archived.machine_id[..len])
            .unwrap_or("<invalid-utf8>");

        // Find or create the row for this machine_id, then update its metrics from the zero-copy archive.
        if let Some(row) = self.rows.iter_mut().find(|r| r.name == machine_id) {
            row.update_from_archived(archived);
        } else {
            // Machine not in config — auto-register it as a new row (auto-discovery).
            log::info!("Auto-discovered new machine '{}' from UDP packet", machine_id);
            let mut row = MachineRow::new(machine_id.to_string(), MachineStatus::Online);
            row.update_from_archived(archived);
            self.rows.push(row);
        }
    }

    /// Mark a machine as offline if no UDP packets received within timeout period.
    pub fn mark_offline(&mut self, machine_id: &str) {
        // TODO: Find row by name and set status to Offline (Beverly implements).
        for row in &mut self.rows {
            if row.name == machine_id {
                row.status = MachineStatus::Offline;
            }
        }
    }

    /// Return the number of online machines currently displayed.
    pub fn online_count(&self) -> usize {
        self.rows.iter()
            .filter(|r| r.status == MachineStatus::Online)
            .count()
    }
}

impl Default for GridWindow {
    fn default() -> Self {
        GridWindow::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grid window initializes empty with no machines (Beverly writes after implementation).
    #[test]
    fn test_grid_empty_init() {
        let grid = GridWindow::new();
        assert!(grid.rows.is_empty(), "New grid should have zero rows");
        assert!(!grid.visible, "Grid should start hidden — only shown on panel click");
    }

    /// Populate from config creates one row per machine with Pending status (Beverly writes).
    #[test]
    fn test_populate_from_config() {
        let mut grid = GridWindow::new();
        let names = vec!["localhost".to_string(), "pluto".to_string(), "spark".to_string()];
        grid.populate_from_config(&names);

        assert_eq!(grid.rows.len(), 3, "Should have one row per configured machine");
        for row in &grid.rows {
            assert_eq!(row.status, MachineStatus::Pending, "All machines start as Pending until first packet");
        }
    }

    /// Online count returns correct number of online machines (Beverly writes).
    #[test]
    fn test_online_count() {
        let mut grid = GridWindow::new();
        grid.populate_from_config(&vec!["a".to_string(), "b".to_string()]);
        assert_eq!(grid.online_count(), 0, "No machines online initially");

        // TODO: Simulate marking machine 'a' as online once UDP receiver is implemented.
    }
}
