use pyo3::prelude::*;
use pythonize::pythonize;

/// Cramér–von Mises goodness-of-fit test against uniform distribution.
#[pyfunction]
fn cramer_von_mises(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::cramer_von_mises(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Ljung-Box autocorrelation test with default lag.
#[pyfunction]
fn ljung_box(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::ljung_box_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Gap test for randomness with default parameters.
#[pyfunction]
fn gap_test(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::gap_test_default(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Run complete statistics analysis suite on entropy stream.
#[pyfunction]
fn statistics_analysis(py: Python<'_>, data: &[u8]) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::statistics_analysis(data);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// One-way ANOVA across multiple byte groups.
#[pyfunction]
fn anova(py: Python<'_>, groups: Vec<Vec<u8>>) -> PyResult<PyObject> {
    let refs: Vec<&[u8]> = groups.iter().map(|g| g.as_slice()).collect();
    let result = openentropy_core::statistics::anova(&refs);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Kruskal-Wallis non-parametric test across multiple byte groups.
#[pyfunction]
fn kruskal_wallis(py: Python<'_>, groups: Vec<Vec<u8>>) -> PyResult<PyObject> {
    let refs: Vec<&[u8]> = groups.iter().map(|g| g.as_slice()).collect();
    let result = openentropy_core::statistics::kruskal_wallis(&refs);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Levene's test for equality of variances across multiple byte groups.
#[pyfunction]
fn levene_test(py: Python<'_>, groups: Vec<Vec<u8>>) -> PyResult<PyObject> {
    let refs: Vec<&[u8]> = groups.iter().map(|g| g.as_slice()).collect();
    let result = openentropy_core::statistics::levene_test(&refs);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Statistical power analysis for a given effect size, sample size, and alpha.
#[pyfunction]
fn power_analysis(
    py: Python<'_>,
    effect_size: f64,
    sample_size: usize,
    alpha: f64,
) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::power_analysis(effect_size, sample_size, alpha);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Bonferroni correction for multiple comparisons.
#[pyfunction]
fn bonferroni_correction(py: Python<'_>, p_values: Vec<f64>, alpha: f64) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::bonferroni_correction(&p_values, alpha);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Holm-Bonferroni stepwise correction for multiple comparisons.
#[pyfunction]
fn holm_bonferroni_correction(
    py: Python<'_>,
    p_values: Vec<f64>,
    alpha: f64,
) -> PyResult<PyObject> {
    let result = openentropy_core::statistics::holm_bonferroni_correction(&p_values, alpha);
    pythonize(py, &result)
        .map(|obj| obj.unbind())
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(cramer_von_mises, m)?)?;
    m.add_function(wrap_pyfunction!(ljung_box, m)?)?;
    m.add_function(wrap_pyfunction!(gap_test, m)?)?;
    m.add_function(wrap_pyfunction!(statistics_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(anova, m)?)?;
    m.add_function(wrap_pyfunction!(kruskal_wallis, m)?)?;
    m.add_function(wrap_pyfunction!(levene_test, m)?)?;
    m.add_function(wrap_pyfunction!(power_analysis, m)?)?;
    m.add_function(wrap_pyfunction!(bonferroni_correction, m)?)?;
    m.add_function(wrap_pyfunction!(holm_bonferroni_correction, m)?)?;
    Ok(())
}
