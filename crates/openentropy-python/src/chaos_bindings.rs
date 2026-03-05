use pyo3::prelude::*;
use pythonize::pythonize;

/// Run all 5 chaos theory analyses on a byte stream.
///
/// Returns a dict with keys: hurst, lyapunov, correlation_dimension,
/// bientropy, epiplexity — each containing the analysis results.
#[pyfunction]
fn chaos_analysis(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::chaos_analysis(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Hurst exponent via R/S analysis.
///
/// H ≈ 0.5 indicates random walk (no long-range dependence).
#[pyfunction]
fn hurst_exponent(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::hurst_exponent(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Lyapunov exponent via Rosenstein algorithm.
///
/// λ ≈ 0 indicates no deterministic chaos.
#[pyfunction]
fn lyapunov_exponent(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::lyapunov_exponent(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Correlation dimension via Grassberger-Procaccia algorithm.
///
/// High D₂ indicates high-dimensional (random) attractor.
#[pyfunction]
fn correlation_dimension(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::correlation_dimension(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// BiEntropy / TBiEntropy binary derivative analysis.
///
/// High values indicate maximal binary entropy.
#[pyfunction]
fn bientropy(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::bientropy(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Epiplexity: compression-based structure detection.
///
/// Compression ratio ≈ 1.0 means incompressible (random).
#[pyfunction]
fn epiplexity(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::epiplexity(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Sample entropy (m=2, r=0.2*std).
///
/// Measures signal complexity/irregularity.
#[pyfunction]
fn sample_entropy(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::sample_entropy_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Detrended fluctuation analysis.
///
/// Detects long-range correlations in non-stationary data.
#[pyfunction]
fn dfa_analysis(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::dfa_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Recurrence quantification analysis.
///
/// Quantifies recurrence structure in the data.
#[pyfunction]
fn rqa_analysis(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::rqa_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Bootstrap confidence intervals for the Hurst exponent.
///
/// Resamples data to estimate H uncertainty.
#[pyfunction]
fn bootstrap_hurst(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::bootstrap_hurst_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Rolling Hurst exponent over sliding windows.
///
/// Tracks how long-range dependence evolves over the data.
#[pyfunction]
fn rolling_hurst(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::chaos::rolling_hurst_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(chaos_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(hurst_exponent, m)?)?;
    m.add_function(wrap_pyfunction!(lyapunov_exponent, m)?)?;
    m.add_function(wrap_pyfunction!(correlation_dimension, m)?)?;
    m.add_function(wrap_pyfunction!(bientropy, m)?)?;
    m.add_function(wrap_pyfunction!(epiplexity, m)?)?;
    m.add_function(wrap_pyfunction!(sample_entropy, m)?)?;
    m.add_function(wrap_pyfunction!(dfa_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(rqa_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(bootstrap_hurst, m)?)?;
    m.add_function(wrap_pyfunction!(rolling_hurst, m)?)?;
    Ok(())
}
