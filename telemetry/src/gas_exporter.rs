use prometheus::{Registry, HistogramVec, register_histogram_vec, Opts};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref GAS_HISTOGRAM: HistogramVec = register_histogram_vec!(
        Opts::new("soroban_gas_usage", "CPU instructions per contract call"),
        &["contract_id", "function_name"]
    ).unwrap();
}

pub fn record_gas_metrics(contract_id: &str, function: &str, gas: u64) {
    GAS_HISTOGRAM
        .with_label_values(&[contract_id, function])
        .observe(gas as f64);
}
