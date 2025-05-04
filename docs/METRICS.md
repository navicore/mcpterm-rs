# Metrics Framework for mcpterm-rs

## Overview

This document outlines the metrics collection system for mcpterm-rs. The system is designed to capture operational metrics and usage patterns with minimal overhead, following the principles of having metrics throughout the product from early development.

## Core Principles

1. **Low Overhead**: Metrics collection should have negligible performance impact
2. **Zero External Dependencies**: Core metrics system uses only standard Rust
3. **Aggregation First**: Metrics are aggregated in memory and reported periodically
4. **Extensible Output**: Support multiple reporting destinations with simple plugins
5. **Privacy Respecting**: Only collect anonymous usage data, never content

## Metric Types

The system supports two primary metric types:

### Counters

Monotonically increasing values that track the number of occurrences:

- Number of LLM API calls
- Number of each MCP command invoked
- Number of errors by type
- Number of user interactions

### Gauges

Point-in-time measurements that can increase or decrease:

- Response latency
- Token counts
- Memory usage
- Active requests

## Implementation

### Core Metrics Registry

A central singleton registry that stores all metrics:

```rust
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, AtomicU64>>,
    gauges: RwLock<HashMap<String, AtomicI64>>,
    last_report_time: AtomicU64,
}

impl MetricsRegistry {
    // Get the global registry instance
    pub fn global() -> &'static Self { /* ... */ }
    
    // Increment a counter by the specified amount (defaults to 1)
    pub fn increment(&self, name: &str, amount: u64) { /* ... */ }
    
    // Set a gauge to the specified value
    pub fn set_gauge(&self, name: &str, value: i64) { /* ... */ }
    
    // Record the duration of an operation
    pub fn record_duration(&self, name: &str, duration: Duration) { /* ... */ }
    
    // Generate a metrics report
    pub fn generate_report(&self) -> MetricsReport { /* ... */ }
    
    // Reset counters after reporting (optional)
    pub fn reset_counters(&self) { /* ... */ }
}
```

### Macros for Ergonomic Usage

Provide macros for ergonomic and concise metrics recording:

```rust
// Increment a counter
count!(api.call.bedrock);

// Set a gauge
gauge!(memory.usage, current_memory_mb);

// Time an operation
time!(api.response_time, {
    let response = client.send_request().await?;
    response
});
```

### Periodic Reporting

Metrics are aggregated and reported at configurable intervals:

```rust
async fn metrics_reporter(interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;
        
        // Generate the report
        let report = MetricsRegistry::global().generate_report();
        
        // Send to configured destinations
        for destination in get_active_destinations() {
            destination.send_report(&report).await;
        }
        
        // Optionally reset counters
        if should_reset_after_report() {
            MetricsRegistry::global().reset_counters();
        }
    }
}
```

### Report Format

Metrics reports are structured for easy parsing and analysis:

```json
{
  "timestamp": "2023-05-01T12:34:56Z",
  "app_version": "0.1.0",
  "session_id": "a1b2c3d4",
  "interval_seconds": 3600,
  "counters": {
    "llm.calls.total": 42,
    "llm.calls.bedrock": 35,
    "llm.calls.anthropic": 7,
    "mcp.command.execute_shell": 15,
    "mcp.command.read_file": 23,
    "error.json_parse": 3,
    "error.network": 1
  },
  "gauges": {
    "latency.p50_ms": 250,
    "latency.p95_ms": 850,
    "memory.usage_mb": 75,
    "tokens.input_avg": 550,
    "tokens.output_avg": 325
  }
}
```

## Key Metrics to Collect

### LLM Interaction
- `llm.calls.total`: Total number of LLM API calls
- `llm.calls.<provider>`: LLM calls by provider
- `llm.tokens.input`: Input tokens consumed
- `llm.tokens.output`: Output tokens generated
- `llm.latency.p50_ms`: Median response time
- `llm.latency.p95_ms`: 95th percentile response time
- `llm.errors.<type>`: Error counts by type
- `llm.retry.count`: Number of automatic retries

### MCP Commands
- `mcp.command.<name>`: Count of each MCP command invoked
- `mcp.command.success_rate`: Percentage of successful commands
- `mcp.json.parse_errors`: JSON parsing errors
- `mcp.json.validation_errors`: Schema validation errors

### User Interaction
- `ui.session.duration`: Length of user sessions
- `ui.input.count`: Number of user inputs
- `ui.command.cancel`: Number of cancelled operations

## Destination Plugins

The system supports multiple reporting destinations through a simple trait:

```rust
pub trait MetricsDestination: Send + Sync {
    async fn send_report(&self, report: &MetricsReport) -> Result<(), MetricsError>;
}
```

### Built-in Destinations

1. **LogDestination**: Write metrics reports to application logs
2. **FileDestination**: Save metrics to a local JSON file
3. **MemoryDestination**: Keep recent metrics in memory for debugging

### Extensible Destinations (Future)

1. **HttpDestination**: Send metrics to a configurable HTTP endpoint
2. **EnterpriseDestination**: Integration with enterprise telemetry systems

## Privacy and Configuration

- All metrics collection respects user privacy
- No content or sensitive information is ever collected
- Users can disable metrics collection entirely
- Configurable reporting intervals and destinations
- Enterprise settings can be configured via configuration file

## Implementation Plan

1. **Phase 1**: Core metrics registry and log destination
2. **Phase 2**: Key LLM and MCP command metrics
3. **Phase 3**: Extended metrics and additional destinations
4. **Phase 4**: User configuration and enterprise integration

## Example Usage

```rust
// Record LLM call
fn send_llm_request(request: &Request) -> Result<Response, LlmError> {
    count!("llm.calls.total");
    count!("llm.calls.bedrock");
    
    let tokens = count_tokens(&request.prompt);
    count!("llm.tokens.input", tokens);
    
    time!("llm.response_time", {
        match client.send_request(request) {
            Ok(response) => {
                count!("llm.tokens.output", count_tokens(&response.content));
                Ok(response)
            }
            Err(e) => {
                count!("llm.errors.total");
                count!(format!("llm.errors.{}", e.error_type()));
                Err(e)
            }
        }
    })
}

// Record MCP command execution
fn execute_mcp_command(cmd: &Command) -> Result<Output, CommandError> {
    count!("mcp.command.total");
    count!(format!("mcp.command.{}", cmd.name));
    
    match execute_command(cmd) {
        Ok(output) => {
            count!("mcp.command.success");
            Ok(output)
        }
        Err(e) => {
            count!("mcp.command.error");
            count!(format!("mcp.command.error.{}", e.error_type()));
            Err(e)
        }
    }
}
```