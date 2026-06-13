use prometheus::{
    Encoder, HistogramVec, IntCounterVec, Gauge, Registry, TextEncoder,
    histogram_opts, opts, register_histogram_vec_with_registry,
    register_int_counter_vec_with_registry, register_gauge_with_registry,
};
use std::sync::Arc;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub struct MetricsRegistry {
    pub registry: Arc<Registry>,
    pub http_requests_histogram: HistogramVec,
    pub http_errors_counter: IntCounterVec,
    pub db_query_duration: HistogramVec,
    pub match_duration_gauge: Gauge,
    pub star_cleaned_counter: IntCounterVec,
}

impl MetricsRegistry {
    pub fn gather(&self) -> prometheus::proto::MetricFamily {
        prometheus::proto::MetricFamily::new()
    }

    pub fn encode_text(&self) -> Result<String, String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)
            .map_err(|e| e.to_string())?;
        String::from_utf8(buffer).map_err(|e| e.to_string())
    }
}

pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let formatting_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(formatting_layer)
        .init();
}

pub fn register_metrics() -> Result<MetricsRegistry, String> {
    let registry = Arc::new(Registry::new());

    let http_requests_histogram = register_histogram_vec_with_registry!(
        histogram_opts!(
            "http_request_duration_seconds",
            "HTTP request duration in seconds",
            vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
        ),
        &["method", "path", "status"],
        registry.as_ref()
    ).map_err(|e| e.to_string())?;

    let http_errors_counter = register_int_counter_vec_with_registry!(
        opts!(
            "http_errors_total",
            "Total number of HTTP errors"
        ),
        &["method", "path", "status"],
        registry.as_ref()
    ).map_err(|e| e.to_string())?;

    let db_query_duration = register_histogram_vec_with_registry!(
        histogram_opts!(
            "db_query_duration_seconds",
            "Database query duration in seconds",
            vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]
        ),
        &["query_type"],
        registry.as_ref()
    ).map_err(|e| e.to_string())?;

    let match_duration_gauge = register_gauge_with_registry!(
        opts!(
            "match_duration_seconds",
            "Last match operation duration in seconds"
        ),
        registry.as_ref()
    ).map_err(|e| e.to_string())?;

    let star_cleaned_counter = register_int_counter_vec_with_registry!(
        opts!(
            "stars_cleaned_total",
            "Total number of stars cleaned"
        ),
        &["dynasty"],
        registry.as_ref()
    ).map_err(|e| e.to_string())?;

    Ok(MetricsRegistry {
        registry,
        http_requests_histogram,
        http_errors_counter,
        db_query_duration,
        match_duration_gauge,
        star_cleaned_counter,
    })
}
