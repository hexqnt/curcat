//! Optimized headless CPU profiling entry point.

#[path = "../workloads/zoom.rs"]
mod workload;

fn main() -> std::process::ExitCode {
    match workload::profile() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
