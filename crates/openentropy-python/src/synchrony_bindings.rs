use pyo3::prelude::*;
use pythonize::pythonize;

/// Mutual information between two entropy streams.
#[pyfunction]
fn mutual_information(py: Python<'_>, data_a: &[u8], data_b: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::synchrony::mutual_information(data_a, data_b);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Phase coherence between two entropy streams.
#[pyfunction]
fn phase_coherence(py: Python<'_>, data_a: &[u8], data_b: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::synchrony::phase_coherence(data_a, data_b);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Cross-synchrony analysis between two entropy streams.
#[pyfunction]
fn cross_sync(py: Python<'_>, data_a: &[u8], data_b: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::synchrony::cross_sync(data_a, data_b);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Complete synchrony analysis between two entropy streams.
#[pyfunction]
fn synchrony_analysis(py: Python<'_>, data_a: &[u8], data_b: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::synchrony::synchrony_analysis(data_a, data_b);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Detect global synchronization events across multiple entropy streams.
#[pyfunction]
fn global_event_detection(py: Python<'_>, streams: Vec<Vec<u8>>) -> PyResult<PyObject> {
    let refs: Vec<&[u8]> = streams.iter().map(|s| s.as_slice()).collect();
    let result = openentropy_core::synchrony::global_event_detection(&refs);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(mutual_information, m)?)?;
    m.add_function(wrap_pyfunction!(phase_coherence, m)?)?;
    m.add_function(wrap_pyfunction!(cross_sync, m)?)?;
    m.add_function(wrap_pyfunction!(synchrony_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(global_event_detection, m)?)?;
    Ok(())
}
