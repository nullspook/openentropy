//! Python bindings for session recording.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use openentropy_core::conditioning::condition;
use openentropy_core::session::{SessionConfig, SessionWriter};
use openentropy_core::source_resolution::{SourceMatchMode, resolve_source_names};

use super::PyEntropyPool;

const DEFAULT_SWEEP_TIMEOUT_SECS: f64 = 10.0;

// ---------------------------------------------------------------------------
// PySessionWriter class
// ---------------------------------------------------------------------------

/// Session writer for recording entropy samples to disk.
#[pyclass(name = "SessionWriter")]
pub struct PySessionWriter {
    inner: Option<SessionWriter>,
}

#[pymethods]
impl PySessionWriter {
    #[new]
    #[pyo3(signature = (sources, output_dir, conditioning="raw", tags=None, note=None, analyze=false))]
    fn new(
        sources: Vec<String>,
        output_dir: &str,
        conditioning: &str,
        tags: Option<HashMap<String, String>>,
        note: Option<String>,
        analyze: bool,
    ) -> PyResult<Self> {
        let mode = super::parse_conditioning_mode(conditioning)?;

        let config = SessionConfig {
            sources,
            conditioning: mode,
            output_dir: PathBuf::from(output_dir),
            tags: tags.unwrap_or_default(),
            note,
            sample_size: 1000,
            include_analysis: analyze,
            ..Default::default()
        };

        let writer =
            SessionWriter::new(config).map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(Self {
            inner: Some(writer),
        })
    }

    /// Write a single sample from a named source.
    fn write_sample(&mut self, source_name: &str, raw: &[u8], conditioned: &[u8]) -> PyResult<()> {
        let writer = self
            .inner
            .as_mut()
            .ok_or_else(|| PyValueError::new_err("session already finished"))?;
        writer
            .write_sample(source_name, raw, conditioned)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Finalize the session and return the session directory path.
    fn finish(&mut self) -> PyResult<String> {
        let writer = self
            .inner
            .take()
            .ok_or_else(|| PyValueError::new_err("session already finished"))?;
        let path = writer
            .finish()
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Total samples recorded so far.
    fn total_samples(&self) -> PyResult<u64> {
        let writer = self
            .inner
            .as_ref()
            .ok_or_else(|| PyValueError::new_err("session already finished"))?;
        Ok(writer.total_samples())
    }

    /// Elapsed seconds since recording started.
    fn elapsed_secs(&self) -> PyResult<f64> {
        let writer = self
            .inner
            .as_ref()
            .ok_or_else(|| PyValueError::new_err("session already finished"))?;
        Ok(writer.elapsed().as_secs_f64())
    }

    /// Path to the session directory.
    fn session_dir(&self) -> PyResult<String> {
        let writer = self
            .inner
            .as_ref()
            .ok_or_else(|| PyValueError::new_err("session already finished"))?;
        Ok(writer.session_dir().to_string_lossy().to_string())
    }
}

// ---------------------------------------------------------------------------
// record() convenience function
// ---------------------------------------------------------------------------

/// Record entropy from a pool for a given duration, returning session metadata as a dict.
#[pyfunction]
#[pyo3(signature = (pool, sources, duration_secs, conditioning="raw", output_dir="sessions", analyze=false))]
fn record(
    py: Python<'_>,
    pool: &PyEntropyPool,
    sources: Vec<String>,
    duration_secs: f64,
    conditioning: &str,
    output_dir: &str,
    analyze: bool,
) -> PyResult<PyObject> {
    let mode = super::parse_conditioning_mode(conditioning)?;
    let max_dur = validate_duration_secs(duration_secs)?;
    let sources = resolve_record_sources(pool, &sources)?;

    let config = SessionConfig {
        sources: sources.clone(),
        conditioning: mode,
        output_dir: PathBuf::from(output_dir),
        sample_size: 1000,
        include_analysis: analyze,
        ..Default::default()
    };

    let writer = SessionWriter::new(config).map_err(|e| PyValueError::new_err(e.to_string()))?;

    let pool_ref = &pool.inner;

    let result = py.allow_threads(|| -> Result<PathBuf, std::io::Error> {
        let mut writer = writer;
        let start = Instant::now();
        let stop_at = start.checked_add(max_dur);

        while !deadline_reached(Instant::now(), stop_at) {
            let sweep_timeout_secs = sweep_timeout_secs(Instant::now(), stop_at);
            if sweep_timeout_secs <= 0.0 {
                break;
            }
            let raw_by_source = pool_ref.collect_enabled_raw_n(&sources, sweep_timeout_secs, 1000);

            for source in &sources {
                let Some(raw) = raw_by_source.get(source) else {
                    continue;
                };
                let conditioned = condition(raw, raw.len(), mode);
                writer.write_sample(source, raw, &conditioned)?;
            }
        }

        writer.finish()
    });

    let session_dir = result.map_err(|e| PyValueError::new_err(e.to_string()))?;

    let json_str = std::fs::read_to_string(session_dir.join("session.json"))
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let json_module = py.import("json")?;
    let parsed = json_module.call_method1("loads", (json_str,))?;
    Ok(parsed.into())
}

fn resolve_record_sources(pool: &PyEntropyPool, requested: &[String]) -> PyResult<Vec<String>> {
    if requested.is_empty() {
        return Err(PyValueError::new_err(
            "at least one source is required for recording",
        ));
    }

    let available = pool.inner.source_names();
    let resolution = resolve_source_names(&available, requested, SourceMatchMode::ExactOnly);

    if !resolution.missing.is_empty() {
        let suffix = if resolution.missing.len() == 1 {
            ""
        } else {
            "s"
        };
        return Err(PyValueError::new_err(format!(
            "unknown source name{suffix}: {}. Use pool.source_names() or pool.sources() to inspect available sources.",
            resolution.missing.join(", ")
        )));
    }

    Ok(resolution.resolved)
}

fn validate_duration_secs(duration_secs: f64) -> PyResult<Duration> {
    if !duration_secs.is_finite() {
        return Err(PyValueError::new_err(
            "duration_secs must be a finite non-negative number",
        ));
    }
    if duration_secs < 0.0 {
        return Err(PyValueError::new_err(
            "duration_secs must be a finite non-negative number",
        ));
    }
    Ok(Duration::from_secs_f64(duration_secs))
}

fn deadline_reached(now: Instant, stop_at: Option<Instant>) -> bool {
    stop_at.is_some_and(|deadline| now >= deadline)
}

fn sweep_timeout_secs(now: Instant, stop_at: Option<Instant>) -> f64 {
    match stop_at {
        Some(deadline) => deadline
            .saturating_duration_since(now)
            .as_secs_f64()
            .min(DEFAULT_SWEEP_TIMEOUT_SECS),
        None => DEFAULT_SWEEP_TIMEOUT_SECS,
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySessionWriter>()?;
    m.add_function(wrap_pyfunction!(record, m)?)?;
    Ok(())
}
