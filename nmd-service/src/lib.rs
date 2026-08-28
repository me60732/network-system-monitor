//! # nmd-service — Library crate root
//!
//! Re-exports types from submodules so they can be used by `cosmic-applet` and tests.
//! The binary entry point (`main.rs`) uses this library crate via the package name.

pub mod config;
pub mod crypto;
mod metrics_aggregator;
pub mod packet;
mod udp_sender;

/// Install/uninstall helpers for the systemd unit file (used by install scripts).
pub mod systemd_unit;

// Re-export public types for ergonomic imports across workspace crates.
pub use config::{DEFAULT_CONFIG_PATH, ServiceConfig};
pub use metrics_aggregator::MetricsAggregator;
pub use packet::{
    ArchivedMetricPacket, MetricPacket, PROTOCOL_VERSION,
    CpuMetrics, GpuMetrics, MemoryMetrics, NetworkMetrics, DiskMetrics, PartitionInfo
};
pub use udp_sender::UdpSender;
