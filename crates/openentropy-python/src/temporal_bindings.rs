use pyo3::prelude::*;
use pythonize::pythonize;

/// Detect change points in entropy stream using CUSUM algorithm.
#[pyfunction]
fn change_point_detection(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::temporal::change_point_detection_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Detect anomalous windows in entropy stream.
#[pyfunction]
fn anomaly_detection(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::temporal::anomaly_detection_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Detect burst patterns in entropy stream.
#[pyfunction]
fn burst_detection(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::temporal::burst_detection_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Detect distributional shifts in entropy stream.
#[pyfunction]
fn shift_detection(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::temporal::shift_detection_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Measure temporal drift in entropy characteristics.
#[pyfunction]
fn temporal_drift(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::temporal::temporal_drift_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Run complete temporal analysis suite on entropy stream.
#[pyfunction]
fn temporal_analysis_suite(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::temporal::temporal_analysis_suite(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Measure stability across multiple entropy sessions.
#[pyfunction]
fn inter_session_stability(py: Python<'_>, sessions: Vec<Vec<u8>>) -> PyResult<PyObject> {
    let refs: Vec<&[u8]> = sessions.iter().map(|s| s.as_slice()).collect();
    let result = openentropy_core::temporal::inter_session_stability(&refs);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(change_point_detection, m)?)?;
    m.add_function(wrap_pyfunction!(anomaly_detection, m)?)?;
    m.add_function(wrap_pyfunction!(burst_detection, m)?)?;
    m.add_function(wrap_pyfunction!(shift_detection, m)?)?;
    m.add_function(wrap_pyfunction!(temporal_drift, m)?)?;
    m.add_function(wrap_pyfunction!(temporal_analysis_suite, m)?)?;
    m.add_function(wrap_pyfunction!(inter_session_stability, m)?)?;
    Ok(())
}
