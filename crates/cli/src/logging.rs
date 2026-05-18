use std::path::PathBuf;

use tracing_subscriber::{
    filter::EnvFilter,
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

/// Logging configuration.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Debug filter string (e.g., "cc_tools=debug,cc_query=trace").
    pub filter: Option<String>,
    /// Whether to output to stderr.
    pub stderr: bool,
    /// Optional file path for debug output.
    pub file: Option<PathBuf>,
    /// Whether verbose mode is enabled.
    pub verbose: bool,
}

impl LoggingConfig {
    /// Create a default logging config.
    pub fn new() -> Self {
        Self {
            filter: None,
            stderr: false,
            file: None,
            verbose: false,
        }
    }

    /// Create from CLI args.
    pub fn from_cli_args(
        debug: &Option<String>,
        debug_to_stderr: bool,
        debug_file: &Option<PathBuf>,
        verbose: bool,
    ) -> Self {
        Self {
            filter: debug.clone(),
            stderr: debug_to_stderr || debug.is_some(),
            file: debug_file.clone(),
            verbose,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize logging with RUST_LOG environment variable support.
pub fn init_logging_with_env(config: &LoggingConfig) -> anyhow::Result<()> {
    // Check RUST_LOG first
    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else if let Some(ref filter_str) = config.filter {
        if filter_str.is_empty() {
            EnvFilter::try_new("debug")?
        } else {
            EnvFilter::try_new(filter_str)?
        }
    } else if config.verbose {
        EnvFilter::try_new("info")?
    } else {
        EnvFilter::try_new("warn")?
    };

    if let Some(ref file_path) = config.file {
        // Output to file
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;

        let file_layer = fmt::layer()
            .with_writer(file)
            .with_target(true)
            .with_ansi(false)
            .compact()
            .with_filter(filter);

        tracing_subscriber::registry().with(file_layer).try_init()?;
    } else {
        // Output to stderr or stdout — use a closure for dynamic dispatch
        let stderr = config.stderr;
        let layer = fmt::layer()
            .with_writer(move || -> Box<dyn std::io::Write + Send> {
                if stderr {
                    Box::new(std::io::stderr())
                } else {
                    Box::new(std::io::stdout())
                }
            })
            .with_target(false)
            .with_ansi(true)
            .compact()
            .with_filter(filter);

        tracing_subscriber::registry().with(layer).try_init()?;
    }

    Ok(())
}
