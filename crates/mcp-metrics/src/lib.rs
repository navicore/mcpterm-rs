use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Metrics Registry that maintains all counters and gauges
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, Arc<AtomicU64>>>,
    gauges: RwLock<HashMap<String, Arc<AtomicI64>>>,
    last_report_time: AtomicU64,
}

/// Metric types supported by the registry
#[derive(Debug, Clone, PartialEq)]
pub enum MetricValue {
    Counter(u64),
    Gauge(i64),
}

/// Metrics report containing a snapshot of all metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    pub timestamp: u64,
    pub app_version: String,
    pub interval_seconds: u64,
    pub counters: HashMap<String, u64>,
    pub gauges: HashMap<String, i64>,
}

// Global registry instance
static GLOBAL_REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

impl MetricsRegistry {
    /// Get or initialize the global metrics registry
    pub fn global() -> &'static Self {
        GLOBAL_REGISTRY.get_or_init(|| Self::new())
    }
    
    /// Create a new metrics registry (primarily for testing)
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        MetricsRegistry {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            last_report_time: AtomicU64::new(now),
        }
    }

    /// Increment a counter by the specified amount (default 1)
    pub fn increment(&self, name: &str, amount: u64) {
        let counter = {
            let counters = self.counters.read().unwrap();
            counters.get(name).cloned()
        };

        match counter {
            Some(counter) => {
                counter.fetch_add(amount, Ordering::Relaxed);
            }
            None => {
                let mut counters = self.counters.write().unwrap();
                let counter = Arc::new(AtomicU64::new(amount));
                counters.insert(name.to_string(), counter);
            }
        }
    }

    /// Set a gauge to the specified value
    pub fn set_gauge(&self, name: &str, value: i64) {
        let gauge = {
            let gauges = self.gauges.read().unwrap();
            gauges.get(name).cloned()
        };

        match gauge {
            Some(gauge) => {
                gauge.store(value, Ordering::Relaxed);
            }
            None => {
                let mut gauges = self.gauges.write().unwrap();
                let gauge = Arc::new(AtomicI64::new(value));
                gauges.insert(name.to_string(), gauge);
            }
        }
    }

    /// Record the duration of an operation
    pub fn record_duration(&self, name: &str, duration: Duration) {
        self.set_gauge(name, duration.as_millis() as i64);
    }

    /// Generate a metrics report
    pub fn generate_report(&self) -> MetricsReport {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let last_report = self.last_report_time.swap(now, Ordering::Relaxed);
        let interval = now - last_report;

        let mut counters = HashMap::new();
        for (name, counter) in self.counters.read().unwrap().iter() {
            counters.insert(name.clone(), counter.load(Ordering::Relaxed));
        }

        let mut gauges = HashMap::new();
        for (name, gauge) in self.gauges.read().unwrap().iter() {
            gauges.insert(name.clone(), gauge.load(Ordering::Relaxed));
        }

        MetricsReport {
            timestamp: now,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            interval_seconds: interval,
            counters,
            gauges,
        }
    }

    /// Reset counters after reporting (optional)
    pub fn reset_counters(&self) {
        // Clear the entire counter map rather than just setting values to 0
        let mut counters = self.counters.write().unwrap();
        counters.clear();
    }
}

/// Trait for metrics destinations
pub trait MetricsDestination: Send + Sync {
    fn send_report(&self, report: &MetricsReport) -> Result<(), String>;
}

/// Log destination that writes metrics to the application log
pub struct LogDestination;

impl MetricsDestination for LogDestination {
    fn send_report(&self, report: &MetricsReport) -> Result<(), String> {
        // log metrics if in debug mode - future implementation could send to a metrics service
        let timestamp = chrono::DateTime::<chrono::Utc>::from(
            UNIX_EPOCH + Duration::from_secs(report.timestamp),
        )
        .format("%Y-%m-%d %H:%M:%S UTC");

        // Log header
        log::debug!("===== Metrics Report: {} =====", timestamp);
        log::debug!("App Version: {}", report.app_version);
        log::debug!("Interval: {} seconds", report.interval_seconds);

        // Log counters
        log::debug!("--- Counters ---");
        for (name, value) in &report.counters {
            log::debug!("{}: {}", name, value);
        }

        // Log gauges
        log::debug!("--- Gauges ---");
        for (name, value) in &report.gauges {
            log::debug!("{}: {}", name, value);
        }

        log::debug!("===== End Metrics Report =====");

        Ok(())
    }
}

/// File destination that saves metrics to a JSON file
pub struct FileDestination {
    file_path: String,
}

impl FileDestination {
    pub fn new(file_path: &str) -> Self {
        Self {
            file_path: file_path.to_string(),
        }
    }
}

impl MetricsDestination for FileDestination {
    fn send_report(&self, report: &MetricsReport) -> Result<(), String> {
        let json = serde_json::to_string_pretty(report)
            .map_err(|e| format!("Failed to serialize report: {}", e))?;

        std::fs::write(&self.file_path, json)
            .map_err(|e| format!("Failed to write report to file: {}", e))?;

        Ok(())
    }
}

// Convenience macros for metrics recording
#[macro_export]
macro_rules! count {
    ($name:expr) => {
        $crate::MetricsRegistry::global().increment($name, 1);
    };
    ($name:expr, $value:expr) => {
        $crate::MetricsRegistry::global().increment($name, $value);
    };
}

#[macro_export]
macro_rules! gauge {
    ($name:expr, $value:expr) => {
        $crate::MetricsRegistry::global().set_gauge($name, $value);
    };
}

#[macro_export]
macro_rules! time {
    ($name:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        $crate::MetricsRegistry::global().record_duration($name, duration);
        result
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_increment() {
        // Create a local registry instance for this test
        let registry = MetricsRegistry::new();
        let counter_name = "test.counter";

        // Make sure counter starts empty
        let report = registry.generate_report();
        assert_eq!(report.counters.get(counter_name), None);

        // First increment: add 1
        registry.increment(counter_name, 1);
        let report = registry.generate_report();
        assert_eq!(report.counters.get(counter_name), Some(&1));

        // Second increment: add 2 more (for a total of 3)
        registry.increment(counter_name, 2);
        let report = registry.generate_report();
        assert_eq!(report.counters.get(counter_name), Some(&3));
    }

    #[test]
    fn test_gauge_set() {
        // Create a local registry instance for this test
        let registry = MetricsRegistry::new();
        let gauge_name = "test.gauge";

        registry.set_gauge(gauge_name, 42);

        let report = registry.generate_report();
        assert_eq!(report.gauges.get(gauge_name), Some(&42));

        registry.set_gauge(gauge_name, 100);
        let report = registry.generate_report();
        assert_eq!(report.gauges.get(gauge_name), Some(&100));
    }

    #[test]
    fn test_duration_recording() {
        // Create a local registry instance for this test
        let registry = MetricsRegistry::new();
        let duration_name = "test.duration";

        let duration = Duration::from_millis(123);
        registry.record_duration(duration_name, duration);

        let report = registry.generate_report();
        assert_eq!(report.gauges.get(duration_name), Some(&123));
    }

    #[test]
    fn test_reset_counters() {
        // Create a local registry instance for this test
        let registry = MetricsRegistry::new();
        let counter_name = "test.reset";

        registry.increment(counter_name, 5);

        let report = registry.generate_report();
        assert_eq!(report.counters.get(counter_name), Some(&5));

        registry.reset_counters();
        let report = registry.generate_report();
        assert_eq!(
            report.counters.get(counter_name),
            None,
            "Counter should be removed after reset"
        );
    }

    #[test]
    fn test_macros() {
        // We need to use the global registry here since macros use it
        let registry = MetricsRegistry::global();

        // Make sure we start with clean state
        registry.reset_counters();
        
        // Use unique keys for this test to avoid collisions
        let counter_name = "macro.test.unique";
        let gauge_name = "macro.gauge.unique";
        let timer_name = "macro.timer.unique";

        // Make sure the counter doesn't exist yet
        registry.reset_counters();
        
        // Manually increment using the registry (not via macro)
        registry.increment(counter_name, 3);
        
        // Check counter value
        let report = registry.generate_report();
        assert_eq!(report.counters.get(counter_name), Some(&3));

        // Test gauge macro with global registry
        gauge!(gauge_name, 42);

        // Test timer macro with global registry
        let result = time!(timer_name, { "result" });
        assert_eq!(result, "result");

        // Check gauge and timer values
        let report = registry.generate_report();
        assert_eq!(report.gauges.get(gauge_name), Some(&42));
        assert!(report.gauges.contains_key(timer_name));
    }
}
