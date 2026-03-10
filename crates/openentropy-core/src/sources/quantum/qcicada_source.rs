//! QCicadaSource — Crypta Labs QCicada USB QRNG.
//!
//! Reads true quantum random bytes from a QCicada USB device via the `qcicada`
//! crate. The hardware generates entropy from photonic shot noise (LED +
//! photodiode), providing full 8 bits/byte of quantum randomness per NIST
//! SP 800-90B.
//!
//! **On-device conditioning**: The QCicada handles its own conditioning internally.
//! No additional conditioning should be applied by the pool when using this source
//! alone. The device supports three modes:
//! - `raw` — health-tested noise after on-device filtering (default)
//! - `sha256` — NIST SP 800-90B SHA-256 conditioning on-device
//! - `samples` — raw ADC readings from the photodiode, no processing
//!
//! Configuration is via environment variables:
//! - `QCICADA_MODE` — post-processing mode: `raw`, `sha256`, or `samples` (default: `raw`)
//! - `QCICADA_POST_PROCESS` — legacy alias for `QCICADA_MODE`
//! - `QCICADA_PORT` — serial port path (auto-detected if unset)
//! - `QCICADA_TIMEOUT` — connection timeout in ms (default: 5000)

use std::sync::{Arc, Mutex, OnceLock};

use crate::source::{EntropySource, Platform, Requirement, SourceCategory, SourceInfo};

/// Thread-safe override for QCicada mode, set by the CLI before source discovery.
/// Checked by `QCicadaConfig::default()` before falling back to env vars.
/// This avoids the need for `unsafe { std::env::set_var(...) }`.
pub static QCICADA_CLI_MODE: OnceLock<String> = OnceLock::new();

static QCICADA_INFO: SourceInfo = SourceInfo {
    name: "qcicada",
    description: "Crypta Labs QCicada USB QRNG \u{2014} quantum shot noise",
    physics: "Photonic shot noise from an LED/photodiode pair inside the QCicada USB device. \
              Photon emission and detection are inherently quantum processes governed by Poisson \
              statistics. The device digitises photodiode current fluctuations to produce true \
              quantum random numbers at full entropy (8 bits/byte) per NIST SP 800-90B.",
    category: SourceCategory::Quantum,
    platform: Platform::Any,
    requirements: &[Requirement::QCicada],
    entropy_rate_estimate: 8.0,
    composite: false,
    is_fast: false, // USB serial init can take up to timeout_ms (default 5s)
};

trait QCicadaDevice: Send {
    fn set_postprocess(&mut self, mode: qcicada::PostProcess) -> Result<(), qcicada::QCicadaError>;
    fn start_continuous_fresh(&mut self) -> Result<(), qcicada::QCicadaError>;
    fn read_continuous(&mut self, n: usize) -> Result<Vec<u8>, qcicada::QCicadaError>;
    fn stop(&mut self) -> Result<(), qcicada::QCicadaError>;
}

impl QCicadaDevice for qcicada::QCicada {
    fn set_postprocess(&mut self, mode: qcicada::PostProcess) -> Result<(), qcicada::QCicadaError> {
        Self::set_postprocess(self, mode)
    }

    fn start_continuous_fresh(&mut self) -> Result<(), qcicada::QCicadaError> {
        Self::start_continuous_fresh(self).map(|_| ())
    }

    fn read_continuous(&mut self, n: usize) -> Result<Vec<u8>, qcicada::QCicadaError> {
        Self::read_continuous(self, n)
    }

    fn stop(&mut self) -> Result<(), qcicada::QCicadaError> {
        Self::stop(self)
    }
}

type DeviceHandle = Box<dyn QCicadaDevice>;
type DeviceOpener =
    dyn Fn(&QCicadaConfig, qcicada::PostProcess) -> Option<DeviceHandle> + Send + Sync;

fn configure_device(device: &mut impl QCicadaDevice, mode: qcicada::PostProcess) -> Option<()> {
    // Preserve prior behavior: best-effort mode set, then require a fresh
    // continuous-mode start before the source is considered open.
    let _ = device.set_postprocess(mode);
    device.start_continuous_fresh().ok()?;
    Some(())
}

fn default_device_opener(
    config: &QCicadaConfig,
    mode: qcicada::PostProcess,
) -> Option<DeviceHandle> {
    let timeout = std::time::Duration::from_millis(config.timeout_ms);
    let port_str = config.port.as_deref();
    let mut qrng = match qcicada::QCicada::open(port_str, Some(timeout)) {
        Ok(q) => q,
        Err(_) => return None,
    };

    configure_device(&mut qrng, mode)?;

    Some(Box::new(qrng))
}

/// Configuration for the QCicada QRNG device.
pub struct QCicadaConfig {
    /// Serial port path (e.g. `/dev/tty.usbmodem*`). `None` means auto-detect.
    pub port: Option<String>,
    /// Connection timeout in milliseconds.
    pub timeout_ms: u64,
    /// Post-processing mode: `"raw"`, `"sha256"`, or `"samples"`.
    pub post_process: String,
}

impl Default for QCicadaConfig {
    fn default() -> Self {
        let port = std::env::var("QCICADA_PORT").ok();
        let timeout_ms = std::env::var("QCICADA_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5000);
        let post_process = QCICADA_CLI_MODE
            .get()
            .cloned()
            .or_else(|| std::env::var("QCICADA_MODE").ok())
            .or_else(|| std::env::var("QCICADA_POST_PROCESS").ok())
            .unwrap_or_else(|| "raw".into());
        Self {
            port,
            timeout_ms,
            post_process,
        }
    }
}

/// Entropy source backed by the QCicada USB QRNG hardware.
pub struct QCicadaSource {
    pub config: QCicadaConfig,
    device: Mutex<Option<DeviceHandle>>,
    available: Mutex<Option<bool>>,
    /// Runtime-mutable mode, initialised from config.post_process.
    mode: Mutex<String>,
    opener: Arc<DeviceOpener>,
}

impl Default for QCicadaSource {
    fn default() -> Self {
        let config = QCicadaConfig::default();
        let mode = config.post_process.clone();
        Self {
            config,
            device: Mutex::new(None),
            available: Mutex::new(None),
            mode: Mutex::new(mode),
            opener: Arc::new(default_device_opener),
        }
    }
}

impl QCicadaSource {
    #[cfg(test)]
    fn with_opener(config: QCicadaConfig, opener: Arc<DeviceOpener>) -> Self {
        let mode = config.post_process.clone();
        Self {
            config,
            device: Mutex::new(None),
            available: Mutex::new(None),
            mode: Mutex::new(mode),
            opener,
        }
    }

    /// Parse the current runtime mode into the crate enum.
    fn post_process_mode(&self) -> qcicada::PostProcess {
        let mode = self.mode.lock().unwrap_or_else(|e| e.into_inner());
        match mode.as_str() {
            "sha256" => qcicada::PostProcess::Sha256,
            "samples" => qcicada::PostProcess::RawSamples,
            _ => qcicada::PostProcess::RawNoise,
        }
    }

    fn stop_device(device: &mut Option<DeviceHandle>) {
        if let Some(qrng) = device.as_mut() {
            let _ = qrng.stop();
        }
        *device = None;
    }

    /// Try to open the QCicada device, set post-processing mode, and switch the
    /// hardware into fresh-start continuous mode so the first read discards
    /// queued bytes and subsequent reads do not drain the device's static
    /// one-shot `ready_bytes` buffer.
    fn try_open(&self) -> Option<DeviceHandle> {
        (self.opener)(&self.config, self.post_process_mode())
    }
}

impl Drop for QCicadaSource {
    fn drop(&mut self) {
        let device = self.device.get_mut().unwrap_or_else(|e| e.into_inner());
        Self::stop_device(device);
    }
}

impl EntropySource for QCicadaSource {
    fn info(&self) -> &SourceInfo {
        &QCICADA_INFO
    }

    fn is_available(&self) -> bool {
        let mut cached = self.available.lock().unwrap_or_else(|e| e.into_inner());
        // Positive result is stable (device was found, assume it stays).
        // Negative result is re-checked each call (device may be hot-plugged).
        if *cached == Some(true) {
            return true;
        }
        let avail = !qcicada::discover_devices().is_empty();
        if avail {
            *cached = Some(true);
        }
        avail
    }

    fn collect(&self, n_samples: usize) -> Vec<u8> {
        // USB serial has limited throughput, so read in moderate chunks. With
        // continuous mode active this is just a serial read, not a fresh START
        // command each time, which avoids the device's static one-shot buffer.
        const CHUNK_SIZE: usize = 8192;

        let mut guard = self.device.lock().unwrap_or_else(|e| e.into_inner());

        // Lazy-init: open device on first call.
        if guard.is_none() {
            *guard = self.try_open();
            if guard.is_none() {
                // USB serial devices need settle time after handle release.
                std::thread::sleep(std::time::Duration::from_millis(500));
                *guard = self.try_open();
            }
        }

        if guard.is_none() {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(n_samples);
        let mut remaining = n_samples;

        while remaining > 0 {
            let chunk = remaining.min(CHUNK_SIZE);
            let read_result = guard.as_mut().unwrap().read_continuous(chunk);
            match read_result {
                Ok(bytes) => {
                    if bytes.is_empty() {
                        break;
                    }
                    remaining -= bytes.len();
                    result.extend_from_slice(&bytes);
                }
                Err(_) => {
                    // Device error — reconnect, restart continuous mode, retry once.
                    Self::stop_device(&mut guard);
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    *guard = self.try_open();
                    match guard.as_mut().map(|q| q.read_continuous(chunk)) {
                        Some(Ok(bytes)) => {
                            if bytes.is_empty() {
                                break;
                            }
                            remaining -= bytes.len();
                            result.extend_from_slice(&bytes);
                        }
                        _ => break,
                    }
                }
            }
        }

        result
    }

    fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
        if key != "mode" {
            return Err(format!("unknown config key: {key}"));
        }
        match value {
            "raw" | "sha256" | "samples" => {}
            _ => {
                return Err(format!(
                    "invalid mode: {value} (expected raw|sha256|samples)"
                ));
            }
        }
        *self.mode.lock().unwrap_or_else(|e| e.into_inner()) = value.to_string();

        // Restart the device on the next collect() so the new mode is applied
        // before continuous reads resume.
        Self::stop_device(&mut self.device.lock().unwrap_or_else(|e| e.into_inner()));
        Ok(())
    }

    fn config_options(&self) -> Vec<(&'static str, String)> {
        vec![(
            "mode",
            self.mode.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct FakeDeviceState {
        set_postprocess_calls: Vec<qcicada::PostProcess>,
        start_continuous_fresh_calls: usize,
        read_requests: Vec<usize>,
        stop_calls: usize,
    }

    struct FakeDevice {
        state: Arc<Mutex<FakeDeviceState>>,
        scripted_reads: VecDeque<Result<Vec<u8>, qcicada::QCicadaError>>,
    }

    impl QCicadaDevice for FakeDevice {
        fn set_postprocess(
            &mut self,
            mode: qcicada::PostProcess,
        ) -> Result<(), qcicada::QCicadaError> {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .set_postprocess_calls
                .push(mode);
            Ok(())
        }

        fn start_continuous_fresh(&mut self) -> Result<(), qcicada::QCicadaError> {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .start_continuous_fresh_calls += 1;
            Ok(())
        }

        fn read_continuous(&mut self, n: usize) -> Result<Vec<u8>, qcicada::QCicadaError> {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .read_requests
                .push(n);
            self.scripted_reads
                .pop_front()
                .unwrap_or_else(|| Ok(vec![0; n]))
        }

        fn stop(&mut self) -> Result<(), qcicada::QCicadaError> {
            self.state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .stop_calls += 1;
            Ok(())
        }
    }

    fn test_config(mode: &str) -> QCicadaConfig {
        QCicadaConfig {
            port: None,
            timeout_ms: 5000,
            post_process: mode.into(),
        }
    }

    fn make_test_source(
        mode: &str,
        devices: Vec<(
            Arc<Mutex<FakeDeviceState>>,
            VecDeque<Result<Vec<u8>, qcicada::QCicadaError>>,
        )>,
        opened_modes: Arc<Mutex<Vec<qcicada::PostProcess>>>,
        open_count: Arc<AtomicUsize>,
    ) -> QCicadaSource {
        let scripted_devices = Arc::new(Mutex::new(VecDeque::from(devices)));
        let opener = Arc::new(move |_config: &QCicadaConfig, mode: qcicada::PostProcess| {
            open_count.fetch_add(1, Ordering::SeqCst);
            opened_modes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(mode);
            let (state, scripted_reads) = scripted_devices
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_front()?;
            let mut device = FakeDevice {
                state,
                scripted_reads,
            };
            configure_device(&mut device, mode)?;
            Some(Box::new(device) as DeviceHandle)
        });
        QCicadaSource::with_opener(test_config(mode), opener)
    }

    #[test]
    fn info() {
        let src = QCicadaSource::default();
        assert_eq!(src.name(), "qcicada");
        assert_eq!(src.info().category, SourceCategory::Quantum);
        assert_eq!(src.info().platform, Platform::Any);
        assert_eq!(src.info().entropy_rate_estimate, 8.0);
        assert!(!src.info().composite);
        assert!(!src.info().is_fast);
        assert_eq!(src.info().requirements, &[Requirement::QCicada]);
    }

    #[test]
    fn config_default() {
        let config = QCicadaConfig {
            port: None,
            timeout_ms: 5000,
            post_process: "raw".into(),
        };
        assert!(config.port.is_none());
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.post_process, "raw");
    }

    #[test]
    fn config_explicit() {
        let config = QCicadaConfig {
            port: Some("/dev/ttyUSB0".into()),
            timeout_ms: 3000,
            post_process: "sha256".into(),
        };
        let src = QCicadaSource::with_opener(config, Arc::new(default_device_opener));
        assert_eq!(src.config.port.as_deref(), Some("/dev/ttyUSB0"));
        assert_eq!(src.config.timeout_ms, 3000);
        assert_eq!(src.config.post_process, "sha256");
    }

    #[test]
    fn post_process_mode_parsing() {
        let src = |mode: &str| {
            QCicadaSource::with_opener(test_config(mode), Arc::new(default_device_opener))
        };
        assert!(matches!(
            src("sha256").post_process_mode(),
            qcicada::PostProcess::Sha256
        ));
        assert!(matches!(
            src("samples").post_process_mode(),
            qcicada::PostProcess::RawSamples
        ));
        assert!(matches!(
            src("raw").post_process_mode(),
            qcicada::PostProcess::RawNoise
        ));
        assert!(matches!(
            src("anything").post_process_mode(),
            qcicada::PostProcess::RawNoise
        ));
    }

    #[test]
    fn set_config_mode() {
        let src = QCicadaSource::default();
        assert!(src.set_config("mode", "sha256").is_ok());
        assert_eq!(src.config_options(), vec![("mode", "sha256".into())]);
        assert!(src.set_config("mode", "samples").is_ok());
        assert_eq!(src.config_options(), vec![("mode", "samples".into())]);
        assert!(src.set_config("mode", "raw").is_ok());
        assert_eq!(src.config_options(), vec![("mode", "raw".into())]);
    }

    #[test]
    fn set_config_invalid() {
        let src = QCicadaSource::default();
        assert!(src.set_config("mode", "invalid").is_err());
        assert!(src.set_config("unknown_key", "raw").is_err());
    }

    #[test]
    fn source_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QCicadaSource>();
    }

    #[test]
    fn collect_uses_continuous_reads_and_chunks_large_requests() {
        let state = Arc::new(Mutex::new(FakeDeviceState::default()));
        let opened_modes = Arc::new(Mutex::new(Vec::new()));
        let open_count = Arc::new(AtomicUsize::new(0));
        let src = make_test_source(
            "raw",
            vec![(
                Arc::clone(&state),
                VecDeque::from([Ok(vec![0xAA; 8192]), Ok(vec![0xBB; 808])]),
            )],
            Arc::clone(&opened_modes),
            Arc::clone(&open_count),
        );

        let data = src.collect(9000);
        let state = state.lock().unwrap_or_else(|e| e.into_inner());

        assert_eq!(data.len(), 9000);
        assert_eq!(open_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            *opened_modes.lock().unwrap_or_else(|e| e.into_inner()),
            vec![qcicada::PostProcess::RawNoise]
        );
        assert_eq!(
            state.set_postprocess_calls,
            vec![qcicada::PostProcess::RawNoise]
        );
        assert_eq!(state.start_continuous_fresh_calls, 1);
        assert_eq!(state.read_requests, vec![8192, 808]);
        assert_eq!(state.stop_calls, 0);
    }

    #[test]
    fn collect_reconnects_and_restarts_continuous_after_read_error() {
        let first_state = Arc::new(Mutex::new(FakeDeviceState::default()));
        let second_state = Arc::new(Mutex::new(FakeDeviceState::default()));
        let opened_modes = Arc::new(Mutex::new(Vec::new()));
        let open_count = Arc::new(AtomicUsize::new(0));
        let src = make_test_source(
            "raw",
            vec![
                (
                    Arc::clone(&first_state),
                    VecDeque::from([Err(qcicada::QCicadaError::Protocol(
                        "simulated read failure".into(),
                    ))]),
                ),
                (
                    Arc::clone(&second_state),
                    VecDeque::from([Ok(vec![0x5A; 64])]),
                ),
            ],
            Arc::clone(&opened_modes),
            Arc::clone(&open_count),
        );

        let data = src.collect(64);
        let first_state = first_state.lock().unwrap_or_else(|e| e.into_inner());
        let second_state = second_state.lock().unwrap_or_else(|e| e.into_inner());

        assert_eq!(data, vec![0x5A; 64]);
        assert_eq!(open_count.load(Ordering::SeqCst), 2);
        assert_eq!(
            *opened_modes.lock().unwrap_or_else(|e| e.into_inner()),
            vec![
                qcicada::PostProcess::RawNoise,
                qcicada::PostProcess::RawNoise
            ]
        );
        assert_eq!(first_state.start_continuous_fresh_calls, 1);
        assert_eq!(first_state.read_requests, vec![64]);
        assert_eq!(first_state.stop_calls, 1);
        assert_eq!(second_state.start_continuous_fresh_calls, 1);
        assert_eq!(second_state.read_requests, vec![64]);
    }

    #[test]
    fn set_config_stops_active_device_and_reopens_with_new_mode() {
        let first_state = Arc::new(Mutex::new(FakeDeviceState::default()));
        let second_state = Arc::new(Mutex::new(FakeDeviceState::default()));
        let opened_modes = Arc::new(Mutex::new(Vec::new()));
        let open_count = Arc::new(AtomicUsize::new(0));
        let src = make_test_source(
            "raw",
            vec![
                (
                    Arc::clone(&first_state),
                    VecDeque::from([Ok(vec![0x11; 4])]),
                ),
                (
                    Arc::clone(&second_state),
                    VecDeque::from([Ok(vec![0x22; 4])]),
                ),
            ],
            Arc::clone(&opened_modes),
            Arc::clone(&open_count),
        );

        assert_eq!(src.collect(4), vec![0x11; 4]);
        assert!(src.set_config("mode", "sha256").is_ok());
        assert!(
            src.device
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none()
        );
        assert_eq!(src.collect(4), vec![0x22; 4]);

        let first_state = first_state.lock().unwrap_or_else(|e| e.into_inner());
        let second_state = second_state.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(first_state.stop_calls, 1);
        assert_eq!(
            *opened_modes.lock().unwrap_or_else(|e| e.into_inner()),
            vec![qcicada::PostProcess::RawNoise, qcicada::PostProcess::Sha256]
        );
        assert_eq!(
            second_state.set_postprocess_calls,
            vec![qcicada::PostProcess::Sha256]
        );
        assert_eq!(open_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn drop_stops_active_device() {
        let state = Arc::new(Mutex::new(FakeDeviceState::default()));
        let opened_modes = Arc::new(Mutex::new(Vec::new()));
        let open_count = Arc::new(AtomicUsize::new(0));
        let src = make_test_source(
            "raw",
            vec![(Arc::clone(&state), VecDeque::from([Ok(vec![0x33; 8])]))],
            opened_modes,
            open_count,
        );

        assert_eq!(src.collect(8), vec![0x33; 8]);
        drop(src);

        let state = state.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(state.stop_calls, 1);
    }

    #[test]
    #[ignore] // Requires QCicada hardware connected via USB
    fn collects_quantum_bytes() {
        let src = QCicadaSource::default();
        if src.is_available() {
            let data = src.collect(64);
            assert!(!data.is_empty());
            assert!(data.len() <= 64);
        }
    }
}
