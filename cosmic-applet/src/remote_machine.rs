//! RemoteMachine - Simplified sensor set fed by UDP packets
//!
//! This module uses simple_sensors (data + rendering only) and feeds them
//! from UDP packets instead of /proc filesystem.
//!
//! Each RemoteMachine instance represents one machine in the network, with its own
//! sensor data and rendering state.

use crate::simple_sensors::RemoteSensors;
use nmd_service::packet::MetricPacket;
use cosmic::Element;

/// A remote machine with simplified sensors
#[derive(Clone)]
pub struct RemoteMachine {
    /// Machine hostname or identifier
    pub name: String,
    
    /// All sensor data (CPU, memory, network, disk, GPU, temperature)
    pub sensors: RemoteSensors,
    
    /// Last update timestamp (seconds since Unix epoch for cloning support)
    pub last_update: u64,
}

impl RemoteMachine {
    pub fn new(name: String) -> Self {
        Self {
            name,
            sensors: RemoteSensors::new(),
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    /// Create a RemoteMachine with fake debug data including partitions
    pub fn new_debug(name: String) -> Self {
        use crate::simple_sensors::PartitionInfo;
        
        let mut machine = Self::new(name);
        
        // Add fake partition data
        machine.sensors.disk.partitions = vec![
            PartitionInfo {
                mount: "/".to_string(),
                total: 500_000_000_000,  // 500 GB
                used: 320_000_000_000,   // 320 GB used (64%)
            },
            PartitionInfo {
                mount: "/home".to_string(),
                total: 1_000_000_000_000,  // 1 TB
                used: 450_000_000_000,     // 450 GB used (45%)
            },
            PartitionInfo {
                mount: "/data".to_string(),
                total: 2_000_000_000_000,  // 2 TB
                used: 1_800_000_000_000,   // 1.8 TB used (90%)
            },
        ];
        
        // Add some fake disk I/O data
        machine.sensors.disk.read_bytes_per_sec = 50_000_000;   // 50 MB/s
        machine.sensors.disk.write_bytes_per_sec = 30_000_000;  // 30 MB/s
        
        // Add fake data for other sensors too
        machine.sensors.cpu.usage_percent = 45.5;
        machine.sensors.memory.used_bytes = 8_589_934_592;  // 8 GB
        machine.sensors.memory.total_bytes = 17_179_869_184; // 16 GB
        machine.sensors.network.rx_bytes_per_sec = 5_000_000;  // 5 MB/s
        machine.sensors.network.tx_bytes_per_sec = 2_000_000;  // 2 MB/s
        machine.sensors.gpu.vram_used_bytes = 2_147_483_648;   // 2 GB
        machine.sensors.gpu.vram_total_bytes = 8_589_934_592;  // 8 GB
        machine.sensors.temperature.celsius = 65.0;
        machine.sensors.uptime_seconds = 86400;  // 1 day
        
        machine
    }
    
    /// Update sensors from incoming UDP packet
    pub fn update_from_packet(&mut self, packet: &MetricPacket) {
        self.sensors.update_from_packet(packet);
        self.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    
    /// Render machine panel with all sensors - clickable to open settings
    pub fn render(&self) -> Element<'static, crate::AppMessage> {
        use cosmic::widget::{column, text, container, button};
        
        let name = self.name.clone();
        let sensors_view = self.sensors.render();
        
        // Wrap in button to make entire machine row clickable
        button::custom(
            container(
                column![
                    text(name.clone()).size(16),
                    sensors_view,
                ]
                .spacing(8)
            )
            .padding(12)
        )
        .on_press(crate::AppMessage::OpenMachineDetail(name))
        .into()
    }
}
