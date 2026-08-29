//! # Integration tests — multi-machine UDP scenarios against real AppState + UdpReceiver.
//!
//! These tests run the actual `UdpReceiver::listen_loop` against loopback UDP sockets and verify
//! that `AppState.machines` reflects incoming authenticated traffic: concurrent multi-machine
//! updates, offline→online transitions, and config changes while live data is flowing.

use cosmic_applet::AppState;
use cosmic_applet::config::manager::{ConfigManager, MachineConfig};
use cosmic_applet::network::test_support::{
    create_test_packet_full, encrypt_packet_ecdh, test_sender_pubkey, unix_now,
};
use cosmic_applet::network::udp_receiver::UdpReceiver;
use cosmic_applet::remote_machine::RemoteMachine;
use cosmic_applet::ui::SettingsWindow;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Offline timeout used by the grid display (RemoteMachine::render checks is_offline(30)).
const OFFLINE_TIMEOUT_SECS: u64 = 30;

/// Build a minimal headless AppState (no cosmic window required — pure state).
fn make_state() -> Arc<RwLock<AppState>> {
    let config_manager = Arc::new(RwLock::new(ConfigManager::default()));
    let pairing_manager = Arc::new(RwLock::new(
        cosmic_applet::pairing_manager::PairingManager::new(std::path::PathBuf::from(
            "/tmp/test_integration_pairing.toml",
        )),
    ));
    // Use Default to get a valid AppState with all required fields
    let mut state: AppState = Default::default();
    // Replace with our custom config and pairing manager
    state.config_manager = config_manager;
    state.pairing_manager = pairing_manager;
    state.machines.clear(); // Remove any default machines
    Arc::new(RwLock::new(state))
}

/// Build a PairingManager with the given machine IDs pre-paired so integration tests
/// bypass TOFU detection (production code still enforces it for real unknown senders).
fn pre_paired_manager(
    machine_ids: &[&str],
) -> Arc<RwLock<cosmic_applet::pairing_manager::PairingManager>> {
    use std::path::PathBuf;
    // Use a unique temp path per process to avoid test interference.
    let path = PathBuf::from(format!(
        "/tmp/test_integration_pairing_{}.toml",
        std::process::id()
    ));
    let mut pm = cosmic_applet::pairing_manager::PairingManager::new(path);
    // Pre-pair each test machine with the real test sender pubkey.
    let dummy_pubkey = test_sender_pubkey();
    for &id in machine_ids {
        let _ = pm.add_pairing(id.to_string(), &dummy_pubkey, "127.0.0.1".to_string());
    }
    Arc::new(RwLock::new(pm))
}

/// Bind a receiver on an ephemeral port, spawn its listen loop against `state`,
/// and return (bound port, receiver pubkey hex, task handle).
async fn spawn_receiver(
    state: Arc<RwLock<AppState>>,
) -> (u16, String, tokio::task::JoinHandle<()>) {
    // Pre-pair all machine IDs used across integration tests so TOFU detection passes.
    let pm = pre_paired_manager(&["spark", "pluto", "saturn", "alpha", "beta", "gamma"]);
    let receiver_pubkey_hex = pm.read().unwrap().get_receiver_x25519_pubkey();
    let receiver = UdpReceiver::new(0, None, pm)
        .await
        .expect("bind UDP receiver");
    let port = receiver.socket.local_addr().expect("local_addr").port();
    let handle = tokio::spawn(async move {
        let mut receiver = receiver;
        receiver.listen_loop(state).await;
    });
    (port, receiver_pubkey_hex, handle)
}

/// Send one encrypted packet for `machine` with the given cpu% and sequence number.
fn send_packet(port: u16, machine: &str, cpu: f32, seq: u32, receiver_pubkey_hex: &str) {
    let packet = create_test_packet_full(
        machine,
        cpu,
        (seq as u64) * 1_000_000, // memory_used encodes the sequence for corruption checks
        8_000_000_000,
        seq,
        unix_now(),
    );
    let buf = encrypt_packet_ecdh(packet, receiver_pubkey_hex);
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind sender socket");
    sock.send_to(&buf, ("127.0.0.1", port))
        .expect("send packet");
}

/// Poll `cond` every 50ms until it returns true or `timeout_ms` elapses.
async fn wait_for(mut cond: impl FnMut() -> bool, timeout_ms: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    while tokio::time::Instant::now() < deadline {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    cond()
}

/// Validates concurrent multi-machine ingestion: 3 machines send 5 packets each simultaneously;
/// all 3 must appear in AppState.machines with the *last* packet's values intact (no loss of the
/// final state, no cross-machine corruption) and per-machine sequences must increment correctly.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_multi_machine_simultaneous_packets() {
    let state = make_state();
    let (port, receiver_pubkey_hex, handle) = spawn_receiver(state.clone()).await;

    let names = ["spark", "pluto", "saturn"];
    let mut senders = Vec::new();
    for name in names {
        let pubkey_hex = receiver_pubkey_hex.clone();
        senders.push(tokio::spawn(async move {
            for seq in 1..=5u32 {
                // cpu encodes the sequence (seq * 10) so the final accepted packet is provable.
                send_packet(port, name, seq as f32 * 10.0, seq, &pubkey_hex);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }));
    }
    for s in senders {
        s.await.expect("sender task");
    }

    // All 3 machines must appear with the final packet (seq 5 → cpu 50.0) applied.
    let ok = wait_for(
        || {
            let st = state.read().unwrap();
            names.iter().all(|n| {
                st.machines
                    .get(*n)
                    .is_some_and(|m| m.sensors.cpu.usage_percent == 50.0)
            })
        },
        5_000,
    )
    .await;
    assert!(
        ok,
        "all 3 machines should reach cpu=50.0 (final packet, seq 5)"
    );

    {
        let st = state.read().unwrap();
        assert_eq!(
            st.machines.len(),
            3,
            "exactly 3 machines in AppState.machines"
        );
        for name in names {
            let m = st.machines.get(name).expect("machine present");
            // memory_used = seq * 1MB proves the seq-5 packet arrived uncorrupted.
            assert_eq!(
                m.sensors.memory.used_bytes, 5_000_000,
                "{name}: last packet data intact"
            );
            assert!(
                !m.is_offline(OFFLINE_TIMEOUT_SECS),
                "{name}: online after fresh packets"
            );
        }
    }

    // Replay a stale sequence (3) for one machine — receiver must reject it, state unchanged.
    send_packet(port, "spark", 99.0, 3, &receiver_pubkey_hex);
    tokio::time::sleep(Duration::from_millis(300)).await;
    {
        let st = state.read().unwrap();
        assert_eq!(
            st.machines["spark"].sensors.cpu.usage_percent, 50.0,
            "stale sequence 3 after 5 must be rejected — sequence tracking is per-machine monotonic"
        );
    }

    handle.abort();
}

/// Validates the offline→online transition: after >30s without packets is_offline() is true
/// (the 35s wait is simulated by backdating last_update — same code path, no real sleep);
/// a fresh packet must flip it back online and update the display-driving sensor data.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_machine_offline_online_transition() {
    let state = make_state();
    let (port, receiver_pubkey_hex, handle) = spawn_receiver(state.clone()).await;

    // Machine "spark" last heard from 35 seconds ago (> 30s offline timeout).
    {
        let mut machine = RemoteMachine::new("spark".to_string());
        machine.last_update -= 35; // simulate the 35-second wait deterministically
        state
            .write()
            .unwrap()
            .machines
            .insert("spark".to_string(), machine);
    }
    {
        let st = state.read().unwrap();
        let m = &st.machines["spark"];
        assert!(
            m.is_offline(OFFLINE_TIMEOUT_SECS),
            "35s-stale machine must report offline"
        );
        assert!(
            m.seconds_since_update() >= 35,
            "staleness clock must reflect the gap"
        );
    }

    // A fresh authenticated packet arrives — machine must come back online with updated metrics.
    send_packet(port, "spark", 77.0, 1, &receiver_pubkey_hex);
    let ok = wait_for(
        || {
            let st = state.read().unwrap();
            st.machines["spark"].sensors.cpu.usage_percent == 77.0
        },
        5_000,
    )
    .await;
    assert!(ok, "fresh packet should update spark's metrics");

    {
        let st = state.read().unwrap();
        let m = &st.machines["spark"];
        assert!(
            !m.is_offline(OFFLINE_TIMEOUT_SECS),
            "machine must be online after fresh packet"
        );
        assert!(
            m.seconds_since_update() < 5,
            "last_update must be refreshed"
        );
        // Display updates: render() derives its offline label from is_offline + these sensors.
        assert_eq!(m.sensors.cpu.usage_percent, 77.0);
    }

    handle.abort();
}

/// Validates config changes under live traffic: with 2 machines streaming, a 3rd machine is
/// added to ConfigManager and the 1st removed (mirroring the applet's Add/RemoveMachine flow);
/// the state that drives the grid display must reflect both changes while data keeps flowing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_config_changes_with_live_data() {
    let state = make_state();
    let (port, receiver_pubkey_hex, handle) = spawn_receiver(state.clone()).await;

    // Two machines streaming live data.
    for seq in 1..=3u32 {
        let pubkey = receiver_pubkey_hex.clone();
        send_packet(port, "alpha", seq as f32, seq, &pubkey);
        send_packet(port, "beta", seq as f32, seq, &pubkey);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let ok = wait_for(
        || {
            let st = state.read().unwrap();
            st.machines.contains_key("alpha") && st.machines.contains_key("beta")
        },
        5_000,
    )
    .await;
    assert!(ok, "both live machines should be registered");

    // Add a 3rd machine to ConfigManager (mirrors the AddMachine flow — no disk write in tests).
    {
        let st = state.read().unwrap();
        let cm = st.config_manager.clone();
        drop(st);
        cm.write().unwrap().machines.push(MachineConfig::new(
            "gamma".to_string(),
            "192.168.1.50".to_string(),
        ));
        let mut st = state.write().unwrap();
        st.machines
            .insert("gamma".to_string(), RemoteMachine::new("gamma".to_string()));
    }

    // Remove the 1st machine (mirrors the RemoveMachine handler: config retain + machines remove).
    {
        let st = state.write().unwrap();
        st.config_manager
            .write()
            .unwrap()
            .machines
            .retain(|m| m.name != "alpha");
        drop(st);
        state.write().unwrap().machines.remove("alpha");
    }

    // Grid data source must reflect the changes: beta + gamma present, alpha gone.
    {
        let st = state.read().unwrap();
        assert!(
            !st.machines.contains_key("alpha"),
            "removed machine gone from grid state"
        );
        assert!(
            st.machines.contains_key("beta"),
            "untouched machine still present"
        );
        assert!(
            st.machines.contains_key("gamma"),
            "added machine present in grid state"
        );

        let cfg = st.config_manager.read().unwrap();
        assert!(
            cfg.machines.iter().any(|m| m.name == "gamma"),
            "gamma in config"
        );
        assert!(
            !cfg.machines.iter().any(|m| m.name == "alpha"),
            "alpha removed from config"
        );
    }

    // Live data continues for the surviving machine and the removed one stays absent.
    send_packet(port, "beta", 44.0, 4, &receiver_pubkey_hex);
    let ok = wait_for(
        || {
            let st = state.read().unwrap();
            st.machines["beta"].sensors.cpu.usage_percent == 44.0
        },
        5_000,
    )
    .await;
    assert!(
        ok,
        "beta should keep receiving live updates after config changes"
    );
    assert!(
        !state.read().unwrap().machines.contains_key("alpha"),
        "alpha must not reappear without new packets"
    );

    handle.abort();
}
