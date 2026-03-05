use pyo3::prelude::*;
use pythonize::pythonize;

#[pyfunction]
fn full_analysis(py: Python<'_>, source_name: &str, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::full_analysis(source_name, data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
#[pyo3(signature = (data, max_lag=128))]
fn autocorrelation_profile(py: Python<'_>, data: &[u8], max_lag: usize) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::autocorrelation_profile(data, max_lag);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn spectral_analysis(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::spectral_analysis(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn bit_bias(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::bit_bias(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn distribution_stats(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::distribution_stats(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn stationarity_test(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::stationarity_test(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn runs_analysis(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::runs_analysis(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn cross_correlation_matrix(
    py: Python<'_>,
    sources_data: Vec<(String, Vec<u8>)>,
) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::cross_correlation_matrix(&sources_data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pyfunction]
fn pearson_correlation(a: &[u8], b: &[u8]) -> f64 {
    openentropy_core::analysis::pearson_correlation(a, b)
}

/// Approximate entropy (m=2, r=0.2*std).
///
/// Quantifies regularity and unpredictability.
#[pyfunction]
fn approximate_entropy(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::approximate_entropy_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Permutation entropy (order=3, delay=1).
///
/// Measures complexity via ordinal patterns.
#[pyfunction]
fn permutation_entropy(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::permutation_entropy_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Anderson-Darling test for uniformity.
///
/// Tests whether byte values follow a uniform distribution.
#[pyfunction]
fn anderson_darling(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::analysis::anderson_darling(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(full_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(autocorrelation_profile, m)?)?;
    m.add_function(wrap_pyfunction!(spectral_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(bit_bias, m)?)?;
    m.add_function(wrap_pyfunction!(distribution_stats, m)?)?;
    m.add_function(wrap_pyfunction!(stationarity_test, m)?)?;
    m.add_function(wrap_pyfunction!(runs_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(cross_correlation_matrix, m)?)?;
    m.add_function(wrap_pyfunction!(pearson_correlation, m)?)?;
    m.add_function(wrap_pyfunction!(approximate_entropy, m)?)?;
    m.add_function(wrap_pyfunction!(permutation_entropy, m)?)?;
    m.add_function(wrap_pyfunction!(anderson_darling, m)?)?;
    Ok(())
}
