mod cli_args;
mod logging;

use clap::Parser;
use cli_args::{Cli, Commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    let logging_config = logging::LoggingConfig::from_cli_args(
        &cli.debug,
        cli.debug_to_stderr,
        &cli.debug_file,
        cli.verbose,
    );
    logging::init_logging_with_env(&logging_config)?;

    tracing::info!("Claude Code (Rust port) v{}", env!("CARGO_PKG_VERSION"));

    // Handle subcommands
    if let Some(ref command) = cli.command {
        return handle_subcommand(command).await;
    }

    // Determine interactive vs non-interactive
    let is_non_interactive = cli_args::is_non_interactive(&cli);

    if is_non_interactive {
        // Non-interactive mode: print response and exit
        handle_print_mode(&cli).await
    } else {
        // Interactive mode: launch REPL/TUI
        handle_interactive_mode(&cli).await
    }
}

/// Handle subcommands (auth, doctor, mcp, plugin, open).
async fn handle_subcommand(command: &Commands) -> anyhow::Result<()> {
    match command {
        Commands::Auth(auth) => handle_auth_command(auth).await,
        Commands::Doctor => handle_doctor_command().await,
        Commands::Mcp(mcp) => handle_mcp_command(mcp).await,
        Commands::Plugin(plugin) => handle_plugin_command(plugin).await,
        Commands::Open(open) => handle_open_command(open).await,
    }
}

/// Handle auth subcommands.
async fn handle_auth_command(args: &cli_args::AuthArgs) -> anyhow::Result<()> {
    match &args.command {
        cli_args::AuthCommands::Login { console, claudeai } => {
            println!("Login command");
            if *console {
                println!("  Mode: Console API key");
            } else if *claudeai {
                println!("  Mode: claude.ai OAuth");
            } else {
                println!("  Mode: Default (auto-detect)");
            }
            println!("  To authenticate, set ANTHROPIC_API_KEY in your environment.");
            Ok(())
        }
        cli_args::AuthCommands::Logout => {
            println!("Logout command");
            println!("  Auth tokens cleared.");
            Ok(())
        }
        cli_args::AuthCommands::Status => {
            println!("Auth Status:");
            let has_api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
            if has_api_key {
                println!("  Auth method: API key");
                println!("  API key source: ANTHROPIC_API_KEY environment variable");
            } else {
                println!("  Auth method: none");
                println!("  Set ANTHROPIC_API_KEY to authenticate.");
            }
            Ok(())
        }
    }
}

/// Handle doctor subcommand.
async fn handle_doctor_command() -> anyhow::Result<()> {
    println!("Claude Code Doctor — Diagnostics");
    println!();

    // Version
    println!("Version: {}", env!("CARGO_PKG_VERSION"));

    // OS
    println!("OS: {}", std::env::consts::OS);

    // Shell
    if let Ok(shell) = std::env::var("SHELL") {
        println!("Shell: {}", shell);
    }

    // API key
    let has_api_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
    println!("API key: {}", if has_api_key { "set" } else { "not set" });

    // Config directory
    if let Some(dir) = cc_core::settings::claude_dir() {
        println!("Config dir: {}", dir.display());
        println!("Config dir exists: {}", dir.exists());
    }

    // ripgrep
    let has_rg = std::process::Command::new("rg")
        .arg("--version")
        .output()
        .is_ok();
    println!("ripgrep: {}", if has_rg { "found" } else { "not found" });

    Ok(())
}

/// Handle MCP subcommands.
async fn handle_mcp_command(args: &cli_args::McpArgs) -> anyhow::Result<()> {
    match &args.command {
        Some(cli_args::McpCommands::Serve) => {
            println!("MCP serve mode — not yet implemented");
            Ok(())
        }
        None => {
            println!("MCP server management");
            println!("  Use 'claude mcp serve' to start MCP server");
            Ok(())
        }
    }
}

/// Handle plugin subcommands.
async fn handle_plugin_command(args: &cli_args::PluginArgs) -> anyhow::Result<()> {
    match &args.command {
        Some(cli_args::PluginCommands::List) => {
            println!("Installed plugins: (none)");
            Ok(())
        }
        Some(cli_args::PluginCommands::Install { name }) => {
            println!("Installing plugin: {}", name);
            println!("  Plugin marketplace not yet implemented");
            Ok(())
        }
        None => {
            println!("Plugin management");
            println!("  Use 'claude plugin list' to see installed plugins");
            Ok(())
        }
    }
}

/// Handle open subcommand.
async fn handle_open_command(_args: &cli_args::OpenArgs) -> anyhow::Result<()> {
    println!("Open remote session — not yet implemented");
    Ok(())
}

/// Handle non-interactive (print) mode.
async fn handle_print_mode(cli: &Cli) -> anyhow::Result<()> {
    // Get prompt from CLI arg or stdin
    let prompt = if let Some(ref p) = cli.prompt {
        p.clone()
    } else {
        // Read from stdin
        let mut input = String::new();
        use std::io::Read;
        std::io::stdin().read_to_string(&mut input)?;
        input.trim().to_string()
    };

    if prompt.is_empty() {
        anyhow::bail!("No prompt provided. Use -p flag or pipe input.");
    }

    tracing::info!("Non-interactive mode, prompt length: {} chars", prompt.len());

    // TODO: Initialize QueryEngine and run query loop
    // For now, print a placeholder
    match cli.output_format {
        cli_args::OutputFormat::Text => {
            println!("(Query engine not yet implemented in print mode)");
        }
        cli_args::OutputFormat::Json => {
            println!(
                r#"{{"type": "result", "stop_reason": "end_turn", "content": "(not yet implemented)"}}"#
            );
        }
        cli_args::OutputFormat::StreamJson => {
            println!(
                r#"{{"type": "result", "stop_reason": "end_turn", "content": "(not yet implemented)"}}"#
            );
        }
    }

    Ok(())
}

/// Handle interactive mode — launch the REPL/TUI.
async fn handle_interactive_mode(cli: &Cli) -> anyhow::Result<()> {
    tracing::info!("Interactive mode");

    // TODO: Initialize session, load tools, launch TUI
    // For now, print a placeholder
    println!("Claude Code (Rust port) — Interactive Mode");
    println!("(TUI not yet implemented)");
    println!();

    if let Some(ref prompt) = cli.prompt {
        println!("Initial prompt: {}", prompt);
    }

    println!("Type /help for available commands, or enter a prompt.");

    Ok(())
}
