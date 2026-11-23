//! High-precision cross-ecosystem benchmark runner for poribesh.
//! Benchmarks .env parsing, variable interpolation, schema validation, and runtime reads.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    hint::black_box,
    path::Path,
    time::Instant,
};

use poribesh::{EnvInterpolator, EnvParser, Rule};
use serde::Serialize;

#[derive(Serialize)]
struct BenchmarkMetric {
    name: String,
    total_ops: usize,
    duration_ms: f64,
    ns_per_op: f64,
    m_ops_per_sec: f64,
}

fn bench<F>(name: &str, mut operation: F, iterations: usize) -> BenchmarkMetric
where
    F: FnMut(),
{
    // Warmup
    for _ in 0..iterations.min(50) {
        operation();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let ns_per_op = (elapsed.as_nanos() as f64) / (iterations as f64);
    let m_ops_per_sec = (iterations as f64 / elapsed.as_secs_f64()) / 1_000_000.0;

    BenchmarkMetric {
        name: name.to_string(),
        total_ops: iterations,
        duration_ms: elapsed_ms,
        ns_per_op,
        m_ops_per_sec,
    }
}

poribesh::env_schema! {
    #[derive(Debug, Clone)]
    pub struct BenchAppEnv {
        pub app_name: String = Rule::string(),
        pub app_env: String = Rule::enum_values(&["development", "production", "test"]),
        pub app_debug: bool = Rule::boolean(),
        pub app_url: String = Rule::url(),
        pub port: u16 = Rule::port(),
        pub db_port: u16 = Rule::port(),
        pub db_pool_min: u32 = Rule::<u32>::integer(),
        pub db_pool_max: u32 = Rule::<u32>::integer(),
        pub redis_port: u16 = Rule::port(),
        pub session_secure: bool = Rule::boolean(),
        pub bcrypt_rounds: u32 = Rule::<u32>::integer(),
        pub mail_port: u16 = Rule::port(),
    }
}

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let corpus_path = Path::new(manifest_dir)
        .join("benches")
        .join("data")
        .join("canonical.env");

    let content = fs::read_to_string(&corpus_path).expect("failed to read canonical.env corpus");

    // 1. Parsing Benchmarks
    let mut parsing_metrics = Vec::new();
    parsing_metrics.push(bench(
        "poribesh_parse_to_map",
        || {
            let map = EnvParser::parse_to_map(black_box(&content)).unwrap();
            black_box(map);
        },
        10_000,
    ));

    parsing_metrics.push(bench(
        "dotenvy_from_read_iter",
        || {
            let mut map = HashMap::new();
            for (k, v) in dotenvy::from_read_iter(black_box(content.as_bytes())).flatten() {
                map.insert(k, v);
            }
            black_box(map);
        },
        10_000,
    ));

    // 2. Interpolation Benchmarks
    let parsed_map = EnvParser::parse_to_map(&content).unwrap();
    let interpolator = EnvInterpolator::new(".");
    let mut interpolation_metrics = Vec::new();
    interpolation_metrics.push(bench(
        "poribesh_interpolate",
        || {
            let res = interpolator.interpolate(black_box(&parsed_map)).unwrap();
            black_box(res);
        },
        10_000,
    ));

    // 3. Schema Validation Benchmarks
    let interpolated_map = interpolator.interpolate(&parsed_map).unwrap();
    let arc_map = std::sync::Arc::new(interpolated_map.clone());
    let validator = BenchAppEnv::validator();
    let mut validation_metrics = Vec::new();

    validation_metrics.push(bench(
        "poribesh_validate_map",
        || {
            let res = validator.validate_arc(black_box(arc_map.clone())).unwrap();
            black_box(res);
        },
        50_000,
    ));

    let validated_env = validator.validate(&interpolated_map).unwrap();
    validation_metrics.push(bench(
        "poribesh_env_schema_from_env",
        || {
            let cfg = BenchAppEnv::from_env(black_box(&validated_env)).unwrap();
            black_box(cfg);
        },
        50_000,
    ));

    // 4. Runtime Read Benchmarks
    let typed_app_env = BenchAppEnv::from_env(&validated_env).unwrap();
    unsafe {
        std::env::set_var("BENCH_PORT", "8080");
    }

    let mut read_metrics = Vec::new();
    read_metrics.push(bench(
        "poribesh_direct_field_get",
        || {
            let port = black_box(typed_app_env.port);
            black_box(port);
        },
        1_000_000,
    ));

    read_metrics.push(bench(
        "poribesh_typed_get",
        || {
            let port = validated_env.get::<u16>(black_box("PORT")).unwrap();
            black_box(port);
        },
        1_000_000,
    ));

    read_metrics.push(bench(
        "poribesh_raw_get",
        || {
            let port = validated_env.get_raw(black_box("PORT")).unwrap();
            black_box(port);
        },
        1_000_000,
    ));

    read_metrics.push(bench(
        "std_env_var",
        || {
            let port = std::env::var(black_box("BENCH_PORT")).unwrap();
            black_box(port);
        },
        500_000,
    ));

    let mut all_results = BTreeMap::new();
    all_results.insert("parsing", parsing_metrics);
    all_results.insert("interpolation", interpolation_metrics);
    all_results.insert("validation", validation_metrics);
    all_results.insert("runtime_read", read_metrics);

    println!("{}", serde_json::to_string_pretty(&all_results).unwrap());
}
