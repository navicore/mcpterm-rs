use mcp_metrics::{count, gauge, time, LogDestination, MetricsDestination, MetricsRegistry};
use std::thread::sleep;
use std::time::Duration;

fn main() {
    // Initialize the metrics system
    env_logger::init();

    // Create a log destination
    let log_dest = LogDestination;

    println!("Starting metrics example...");

    // Simulate application activity
    for i in 1..=5 {
        println!("Iteration {}", i);

        // Record LLM API calls
        count!("llm.calls.total");
        count!("llm.calls.bedrock");
        count!("llm.tokens.input", 150 + i * 10);
        count!("llm.tokens.output", 200 + i * 20);

        // Record MCP commands
        count!("mcp.command.total");
        count!("mcp.command.execute_shell");

        if i % 3 == 0 {
            count!("mcp.command.read_file");
        }

        if i % 4 == 0 {
            count!("error.json_parse");
        }

        // Record latency metrics
        time!("llm.response_time", {
            sleep(Duration::from_millis(50 + i * 10));
        });

        // Set gauge values
        gauge!("memory.usage_mb", (50 + i * 5).try_into().unwrap());

        // Sleep between iterations
        sleep(Duration::from_millis(100));
    }

    // Generate and display metrics report
    let report = MetricsRegistry::global().generate_report();
    log_dest.send_report(&report).unwrap();

    println!("\nMetrics Report:\n");
    println!("Counters:");
    for (name, value) in &report.counters {
        println!("  {}: {}", name, value);
    }

    println!("\nGauges:");
    for (name, value) in &report.gauges {
        println!("  {}: {}", name, value);
    }

    println!("\nExample complete!");
}
