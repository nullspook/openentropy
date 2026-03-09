//! HTTP entropy server with an ANU-style random endpoint.
//!
//! Serves random bytes via HTTP with an ANU-style JSON shape for easy integration with QRNG
//! clients while preserving OpenEntropy's explicit byte-length semantics.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
    response::Json,
    routing::get,
};
use openentropy_core::conditioning::ConditioningMode;
use openentropy_core::pool::EntropyPool;
use openentropy_core::pool::HealthReport;
use openentropy_core::telemetry::{
    TelemetryWindowReport, collect_telemetry_snapshot, collect_telemetry_window,
};
use serde::{Deserialize, Serialize};

/// Shared server state.
///
/// `EntropyPool` uses interior mutability (`Mutex<Vec<u8>>`, `Mutex<[u8;32]>`, etc.)
/// so all its methods take `&self`. No outer mutex needed — concurrent HTTP requests
/// can access the pool simultaneously without serializing.
struct AppState {
    pool: EntropyPool,
    allow_raw: bool,
}

#[derive(Deserialize)]
struct RandomParams {
    length: Option<usize>,
    #[serde(rename = "type")]
    data_type: Option<String>,
    /// If true, return raw unconditioned entropy (no SHA-256/DRBG).
    raw: Option<bool>,
    /// Conditioning mode: raw, vonneumann, sha256 (overrides `raw` flag).
    conditioning: Option<String>,
    /// Request entropy from a specific source by name.
    source: Option<String>,
}

#[derive(Serialize)]
struct RandomResponse {
    #[serde(rename = "type")]
    data_type: String,
    /// Number of output bytes represented by `data`.
    length: usize,
    /// Number of values in the `data` array after encoding for `type`.
    value_count: usize,
    data: serde_json::Value,
    success: bool,
    /// Whether this output was conditioned (SHA-256) or raw.
    conditioned: bool,
    /// Which source was queried (null if mixed pool).
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    /// Error message if request failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    sources_healthy: usize,
    sources_total: usize,
    raw_bytes: u64,
    output_bytes: u64,
}

#[derive(Serialize)]
struct ApiErrorResponse {
    success: bool,
    error: String,
}

#[derive(Serialize)]
struct SourcesResponse {
    sources: Vec<SourceEntry>,
    total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    telemetry_v1: Option<TelemetryWindowReport>,
}

#[derive(Serialize)]
struct PoolStatusResponse {
    sources_healthy: usize,
    total: usize,
    raw_bytes: u64,
    output_bytes: u64,
    buffer_size: usize,
    sources: Vec<SourceEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    telemetry_v1: Option<TelemetryWindowReport>,
}

#[derive(Serialize)]
struct SourceEntry {
    name: String,
    healthy: bool,
    bytes: u64,
    entropy: f64,
    min_entropy: f64,
    autocorrelation: f64,
    time: f64,
    failures: u64,
}

#[derive(Deserialize, Default)]
struct DiagnosticsParams {
    telemetry: Option<bool>,
}

fn include_telemetry(params: &DiagnosticsParams) -> bool {
    params.telemetry.unwrap_or(false)
}

async fn handle_random(
    State(state): State<Arc<AppState>>,
    params: Result<Query<RandomParams>, QueryRejection>,
) -> (StatusCode, Json<RandomResponse>) {
    let params = match params {
        Ok(Query(params)) => params,
        Err(err) => return random_query_error_response(err),
    };

    let length = params.length.unwrap_or(1024);
    let data_type = params.data_type.unwrap_or_else(|| "hex16".to_string());
    if !matches!(data_type.as_str(), "hex16" | "uint8" | "uint16") {
        return Json(RandomResponse {
            data_type,
            length: 0,
            value_count: 0,
            data: serde_json::Value::Array(vec![]),
            success: false,
            conditioned: false,
            source: params.source.clone(),
            error: Some("Invalid type. Expected one of: hex16, uint8, uint16.".to_string()),
        })
        .with_status(StatusCode::BAD_REQUEST);
    }
    if let Err(error) = validate_request_length(length, &data_type) {
        return Json(RandomResponse {
            data_type,
            length: 0,
            value_count: 0,
            data: serde_json::Value::Array(vec![]),
            success: false,
            conditioned: false,
            source: params.source.clone(),
            error: Some(error),
        })
        .with_status(StatusCode::BAD_REQUEST);
    }

    // Determine conditioning mode: ?conditioning= takes priority, then ?raw=true
    let mode = if let Some(ref c) = params.conditioning {
        match c.as_str() {
            "raw" if state.allow_raw => ConditioningMode::Raw,
            "raw" => {
                return Json(RandomResponse {
                    data_type,
                    length: 0,
                    value_count: 0,
                    data: serde_json::Value::Array(vec![]),
                    success: false,
                    conditioned: false,
                    source: params.source.clone(),
                    error: Some("Raw conditioning is not enabled. Start the server with --allow-raw to permit unconditioned output.".to_string()),
                })
                .with_status(StatusCode::FORBIDDEN);
            }
            "vonneumann" | "von_neumann" | "vn" => ConditioningMode::VonNeumann,
            "sha256" => ConditioningMode::Sha256,
            other => {
                return Json(RandomResponse {
                    data_type,
                    length: 0,
                    value_count: 0,
                    data: serde_json::Value::Array(vec![]),
                    success: false,
                    conditioned: false,
                    source: params.source.clone(),
                    error: Some(format!(
                        "Invalid conditioning mode '{other}'. Expected one of: sha256, vonneumann|von_neumann|vn, raw."
                    )),
                })
                .with_status(StatusCode::BAD_REQUEST);
            }
        }
    } else if params.raw.unwrap_or(false) {
        if state.allow_raw {
            ConditioningMode::Raw
        } else {
            return Json(RandomResponse {
                data_type,
                length: 0,
                value_count: 0,
                data: serde_json::Value::Array(vec![]),
                success: false,
                conditioned: false,
                source: params.source.clone(),
                error: Some("Raw output is not enabled. Start the server with --allow-raw to permit unconditioned output.".to_string()),
            })
            .with_status(StatusCode::FORBIDDEN);
        }
    } else {
        ConditioningMode::Sha256
    };

    let raw = if let Some(ref source_name) = params.source {
        match state.pool.get_source_bytes(source_name, length, mode) {
            Some(bytes) => bytes,
            None => {
                let err_msg = format!(
                    "Unknown source: {source_name}. Use /sources to list available sources."
                );
                return Json(RandomResponse {
                    data_type,
                    length: 0,
                    value_count: 0,
                    data: serde_json::Value::Array(vec![]),
                    success: false,
                    conditioned: mode != ConditioningMode::Raw,
                    source: Some(source_name.clone()),
                    error: Some(err_msg),
                })
                .with_status(StatusCode::BAD_REQUEST);
            }
        }
    } else {
        state.pool.get_bytes(length, mode)
    };
    let use_raw = mode == ConditioningMode::Raw;

    let (data, value_count) = serialize_random_data(&raw, &data_type);

    (
        StatusCode::OK,
        Json(RandomResponse {
            data_type,
            length: raw.len(),
            value_count,
            data,
            success: true,
            conditioned: !use_raw,
            source: params.source,
            error: None,
        }),
    )
}

fn validate_request_length(length: usize, data_type: &str) -> Result<(), String> {
    if !(1..=65536).contains(&length) {
        return Err(format!(
            "Invalid length {length}. Expected a byte count in the range 1..=65536."
        ));
    }
    if matches!(data_type, "hex16" | "uint16") && !length.is_multiple_of(2) {
        return Err(format!(
            "type={data_type} requires an even byte length because values are encoded as 16-bit words."
        ));
    }
    Ok(())
}

fn serialize_random_data(raw: &[u8], data_type: &str) -> (serde_json::Value, usize) {
    match data_type {
        "hex16" => {
            let hex_pairs: Vec<String> = raw
                .chunks_exact(2)
                .map(|c| format!("{:02x}{:02x}", c[0], c[1]))
                .collect();
            let value_count = hex_pairs.len();
            (
                serde_json::Value::Array(
                    hex_pairs
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
                value_count,
            )
        }
        "uint8" => (
            serde_json::Value::Array(raw.iter().map(|&b| serde_json::Value::from(b)).collect()),
            raw.len(),
        ),
        "uint16" => {
            let vals: Vec<u16> = raw
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let value_count = vals.len();
            (
                serde_json::Value::Array(vals.into_iter().map(serde_json::Value::from).collect()),
                value_count,
            )
        }
        _ => unreachable!("validated above"),
    }
}

trait JsonWithStatus<T> {
    fn with_status(self, status: StatusCode) -> (StatusCode, Json<T>);
}

impl<T> JsonWithStatus<T> for Json<T> {
    fn with_status(self, status: StatusCode) -> (StatusCode, Json<T>) {
        (status, self)
    }
}

fn query_error_message(err: QueryRejection) -> String {
    format!("Invalid query parameters: {err}")
}

fn random_query_error_response(err: QueryRejection) -> (StatusCode, Json<RandomResponse>) {
    Json(RandomResponse {
        data_type: "hex16".to_string(),
        length: 0,
        value_count: 0,
        data: serde_json::Value::Array(vec![]),
        success: false,
        conditioned: false,
        source: None,
        error: Some(query_error_message(err)),
    })
    .with_status(StatusCode::BAD_REQUEST)
}

fn api_query_error_response(err: QueryRejection) -> (StatusCode, Json<ApiErrorResponse>) {
    Json(ApiErrorResponse {
        success: false,
        error: query_error_message(err),
    })
    .with_status(StatusCode::BAD_REQUEST)
}

fn source_entries(report: &HealthReport) -> Vec<SourceEntry> {
    report
        .sources
        .iter()
        .map(|s| SourceEntry {
            name: s.name.clone(),
            healthy: s.healthy,
            bytes: s.bytes,
            entropy: s.entropy,
            min_entropy: s.min_entropy,
            autocorrelation: s.autocorrelation,
            time: s.time,
            failures: s.failures,
        })
        .collect()
}

async fn handle_health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let report = state.pool.health_report();
    Json(HealthResponse {
        status: if report.healthy > 0 {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
        sources_healthy: report.healthy,
        sources_total: report.total,
        raw_bytes: report.raw_bytes,
        output_bytes: report.output_bytes,
    })
}

async fn handle_sources(
    State(state): State<Arc<AppState>>,
    params: Result<Query<DiagnosticsParams>, QueryRejection>,
) -> Result<Json<SourcesResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let params = match params {
        Ok(Query(params)) => params,
        Err(err) => return Err(api_query_error_response(err)),
    };
    let telemetry_start = include_telemetry(&params).then(collect_telemetry_snapshot);
    let report = state.pool.health_report();
    let telemetry_v1 = telemetry_start.map(collect_telemetry_window);
    let sources = source_entries(&report);
    let total = sources.len();
    Ok(Json(SourcesResponse {
        sources,
        total,
        telemetry_v1,
    }))
}

async fn handle_pool_status(
    State(state): State<Arc<AppState>>,
    params: Result<Query<DiagnosticsParams>, QueryRejection>,
) -> Result<Json<PoolStatusResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    let params = match params {
        Ok(Query(params)) => params,
        Err(err) => return Err(api_query_error_response(err)),
    };
    let telemetry_start = include_telemetry(&params).then(collect_telemetry_snapshot);
    let report = state.pool.health_report();
    Ok(Json(PoolStatusResponse {
        sources_healthy: report.healthy,
        total: report.total,
        raw_bytes: report.raw_bytes,
        output_bytes: report.output_bytes,
        buffer_size: report.buffer_size,
        sources: source_entries(&report),
        telemetry_v1: telemetry_start.map(collect_telemetry_window),
    }))
}

async fn handle_index(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let source_names = state.pool.source_names();

    Json(serde_json::json!({
        "name": "OpenEntropy Server",
        "version": openentropy_core::VERSION,
        "sources": source_names.len(),
        "endpoints": {
            "/": "This API index",
            "/api/v1/random": {
                "method": "GET",
                "description": "Get random entropy bytes",
                "params": {
                    "length": "Number of output bytes (1-65536, default: 1024). type=hex16 and type=uint16 require an even byte length.",
                    "type": "Output encoding: hex16, uint8, uint16 (default: hex16). hex16/uint16 pack 2 bytes per array value.",
                    "source": format!("Request from a specific source by name. Available: {}", source_names.join(", ")),
                    "conditioning": "Conditioning mode: sha256 (default), vonneumann, raw",
                },
                "response_fields": {
                    "length": "Returned byte count represented by data",
                    "value_count": "Number of encoded values in the data array",
                    "data": "Entropy payload encoded according to type"
                }
            },
            "/sources": {
                "description": "List all active entropy sources with health metrics",
                "params": {
                    "telemetry": "Include telemetry_v1 start/end report (true/false, default false)"
                },
                "response_fields": {
                    "total": "Total number of source entries in the response",
                    "sources": "Per-source health rows with name, healthy(boolean), bytes, entropy, min_entropy, autocorrelation, time, failures",
                    "telemetry_v1": "Optional telemetry window when telemetry=true"
                }
            },
            "/pool/status": {
                "description": "Detailed pool status",
                "params": {
                    "telemetry": "Include telemetry_v1 start/end report (true/false, default false)"
                },
                "response_fields": {
                    "sources_healthy": "Number of currently healthy sources in the pool",
                    "total": "Total number of registered sources",
                    "raw_bytes": "Total raw bytes collected across sources",
                    "output_bytes": "Total conditioned output bytes produced",
                    "buffer_size": "Current pool buffer size in bytes",
                    "sources": "Per-source health rows with name, healthy(boolean), bytes, entropy, min_entropy, autocorrelation, time, failures",
                    "telemetry_v1": "Optional telemetry window when telemetry=true"
                }
            },
            "/health": {
                "description": "Health check",
                "response_fields": {
                    "status": "healthy when one or more sources are healthy, degraded otherwise",
                    "sources_healthy": "Number of currently healthy sources",
                    "sources_total": "Total number of registered sources",
                    "raw_bytes": "Total raw bytes collected across sources",
                    "output_bytes": "Total conditioned output bytes produced"
                }
            },
        },
        "error_contract": "Invalid query parameters return JSON 400 responses",
        "examples": {
            "mixed_pool": "/api/v1/random?length=32&type=uint8",
            "single_source": format!("/api/v1/random?length=32&source={}", source_names.first().map(|s| s.as_str()).unwrap_or("clock_jitter")),
            "raw_output": "/api/v1/random?length=32&conditioning=raw",
            "sources_with_telemetry": "/sources?telemetry=true",
            "pool_with_telemetry": "/pool/status?telemetry=true",
        }
    }))
}

/// Build the axum router.
fn build_router(pool: EntropyPool, allow_raw: bool) -> Router {
    let state = Arc::new(AppState { pool, allow_raw });

    Router::new()
        .route("/", get(handle_index))
        .route("/api/v1/random", get(handle_random))
        .route("/health", get(handle_health))
        .route("/sources", get(handle_sources))
        .route("/pool/status", get(handle_pool_status))
        .with_state(state)
}

/// Run the HTTP entropy server.
///
/// Returns an error if the address cannot be bound or the server encounters
/// a fatal I/O error.
pub async fn run_server(
    pool: EntropyPool,
    host: &str,
    port: u16,
    allow_raw: bool,
) -> std::io::Result<()> {
    let app = build_router(pool, allow_raw);
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::{
        AppState, DiagnosticsParams, RandomParams, build_router, handle_random, include_telemetry,
    };
    use axum::{
        body::{Body, to_bytes},
        extract::{Query, State},
        http::{Request, StatusCode},
    };
    use openentropy_core::pool::EntropyPool;
    use openentropy_core::source::{
        EntropySource, Platform, Requirement, SourceCategory, SourceInfo,
    };
    use serde_json::Value;
    use tower::util::ServiceExt;

    struct TestSource {
        info: SourceInfo,
    }

    impl TestSource {
        fn new() -> Self {
            Self {
                info: SourceInfo {
                    name: "test_source",
                    description: "test source",
                    physics: "deterministic test bytes",
                    category: SourceCategory::System,
                    platform: Platform::Any,
                    requirements: &[] as &[Requirement],
                    entropy_rate_estimate: 1.0,
                    composite: false,
                    is_fast: true,
                },
            }
        }
    }

    impl EntropySource for TestSource {
        fn info(&self) -> &SourceInfo {
            &self.info
        }

        fn is_available(&self) -> bool {
            true
        }

        fn collect(&self, n_samples: usize) -> Vec<u8> {
            vec![0xAA; n_samples]
        }
    }

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            pool: EntropyPool::new(None),
            allow_raw: false,
        })
    }

    fn test_router() -> axum::Router {
        let mut pool = EntropyPool::new(Some(b"server-test"));
        pool.add_source(Box::new(TestSource::new()));
        build_router(pool, false)
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body bytes");
        serde_json::from_slice(&bytes).expect("valid json body")
    }

    fn assert_source_entry_schema(source: &Value) {
        let obj = source.as_object().expect("source row object");
        let keys: BTreeSet<_> = obj.keys().map(String::as_str).collect();
        let expected = BTreeSet::from([
            "autocorrelation",
            "bytes",
            "entropy",
            "failures",
            "healthy",
            "min_entropy",
            "name",
            "time",
        ]);

        assert_eq!(keys, expected);
        assert!(source["name"].is_string());
        assert!(source["healthy"].is_boolean());
        assert!(source["bytes"].is_u64());
        assert!(source["entropy"].is_number());
        assert!(source["min_entropy"].is_number());
        assert!(source["autocorrelation"].is_number());
        assert!(source["time"].is_number());
        assert!(source["failures"].is_u64());
    }

    #[test]
    fn telemetry_flag_defaults_to_false() {
        let default = DiagnosticsParams::default();
        assert!(!include_telemetry(&default));
        assert!(include_telemetry(&DiagnosticsParams {
            telemetry: Some(true),
        }));
    }

    #[tokio::test]
    async fn invalid_conditioning_returns_bad_request() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("uint8".to_string()),
                raw: None,
                conditioning: Some("bogus".to_string()),
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("Invalid conditioning mode"))
        );
    }

    #[tokio::test]
    async fn invalid_type_returns_bad_request() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("hex".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("Invalid type"))
        );
    }

    #[tokio::test]
    async fn unknown_source_returns_bad_request() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("uint8".to_string()),
                raw: None,
                conditioning: None,
                source: Some("definitely_not_a_source".to_string()),
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert_eq!(body.0.source.as_deref(), Some("definitely_not_a_source"));
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("Unknown source"))
        );
    }

    #[tokio::test]
    async fn raw_conditioning_requires_allow_raw() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("uint8".to_string()),
                raw: None,
                conditioning: Some("raw".to_string()),
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("--allow-raw"))
        );
    }

    #[tokio::test]
    async fn uint8_length_reports_bytes_and_value_count() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("uint8".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.0.success);
        assert_eq!(body.0.length, 32);
        assert_eq!(body.0.value_count, 32);
        assert_eq!(body.0.data.as_array().map(Vec::len), Some(32));
    }

    #[tokio::test]
    async fn uint16_length_reports_bytes_and_word_count() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("uint16".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.0.success);
        assert_eq!(body.0.length, 32);
        assert_eq!(body.0.value_count, 16);
        assert_eq!(body.0.data.as_array().map(Vec::len), Some(16));
    }

    #[tokio::test]
    async fn hex16_length_reports_bytes_and_word_count() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(32),
                data_type: Some("hex16".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.0.success);
        assert_eq!(body.0.length, 32);
        assert_eq!(body.0.value_count, 16);
        assert_eq!(body.0.data.as_array().map(Vec::len), Some(16));
        assert!(body.0.data.as_array().is_some_and(|items| {
            items
                .iter()
                .all(|value| value.as_str().is_some_and(|s| s.len() == 4))
        }));
    }

    #[tokio::test]
    async fn uint16_rejects_odd_byte_lengths() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(31),
                data_type: Some("uint16".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("even byte length"))
        );
    }

    #[tokio::test]
    async fn hex16_rejects_odd_byte_lengths() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(31),
                data_type: Some("hex16".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("even byte length"))
        );
    }

    #[tokio::test]
    async fn length_zero_returns_bad_request() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(0),
                data_type: Some("uint8".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("range 1..=65536"))
        );
    }

    #[tokio::test]
    async fn length_above_max_returns_bad_request() {
        let state = test_state();

        let (status, body) = handle_random(
            State(state),
            Ok(Query(RandomParams {
                length: Some(65_537),
                data_type: Some("uint8".to_string()),
                raw: None,
                conditioning: None,
                source: None,
            })),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!body.0.success);
        assert!(
            body.0
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("range 1..=65536"))
        );
    }

    #[tokio::test]
    async fn random_route_invalid_query_returns_json_bad_request() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/random?length=nope")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok());
        assert_eq!(content_type, Some("application/json"));

        let body = response_json(response).await;
        assert_eq!(body["success"], Value::Bool(false));
        assert_eq!(body["length"], Value::from(0));
        assert_eq!(body["value_count"], Value::from(0));
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|msg| msg.contains("Invalid query parameters"))
        );
    }

    #[tokio::test]
    async fn sources_route_invalid_query_returns_json_bad_request() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/sources?telemetry=nope")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok());
        assert_eq!(content_type, Some("application/json"));

        let body = response_json(response).await;
        assert_eq!(body["success"], Value::Bool(false));
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|msg| msg.contains("Invalid query parameters"))
        );
    }

    #[tokio::test]
    async fn sources_route_returns_expected_schema_with_telemetry() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/sources?telemetry=true")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let sources = body["sources"].as_array().expect("sources array");
        assert_eq!(body["total"].as_u64(), Some(sources.len() as u64));
        if let Some(source) = sources.first() {
            assert_source_entry_schema(source);
        }
        assert!(body.get("telemetry_v1").is_some());
    }

    #[tokio::test]
    async fn pool_status_route_invalid_query_returns_json_bad_request() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/pool/status?telemetry=nope")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok());
        assert_eq!(content_type, Some("application/json"));

        let body = response_json(response).await;
        assert_eq!(body["success"], Value::Bool(false));
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|msg| msg.contains("Invalid query parameters"))
        );
    }

    #[tokio::test]
    async fn pool_status_route_uses_sources_healthy_and_includes_telemetry() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .uri("/pool/status?telemetry=true")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let sources = body["sources"].as_array().expect("sources array");
        let healthy_count = sources
            .iter()
            .filter(|source| source["healthy"].as_bool() == Some(true))
            .count() as u64;

        assert_eq!(body["sources_healthy"].as_u64(), Some(healthy_count));
        assert_eq!(body["total"].as_u64(), Some(sources.len() as u64));
        if let Some(source) = sources.first() {
            assert_source_entry_schema(source);
        }
        assert!(body.get("healthy").is_none());
        assert!(body.get("telemetry_v1").is_some());
    }
}
