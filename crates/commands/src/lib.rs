pub mod traits;
pub mod registry;
pub mod executor;
pub mod tier1;
pub mod tier2;
pub mod tier3;
pub mod utility;

pub use traits::{
    Command, CommandType, CommandResult, CommandResultDisplay, CommandContext,
    AuthType, meets_availability, get_command_name, is_command_enabled,
};
pub use registry::CommandRegistry;
pub use executor::{parse_slash_command, execute_command, is_slash_command, get_suggestions};

/// Register all commands into the registry.
pub fn register_all_commands(registry: &mut CommandRegistry) {
    // Tier 1
    registry.register(std::sync::Arc::new(tier1::help::HelpCommand::new()));
    registry.register(std::sync::Arc::new(tier1::clear::ClearCommand::new()));
    registry.register(std::sync::Arc::new(tier1::compact::CompactCommand::new()));
    registry.register(std::sync::Arc::new(tier1::config::ConfigCommand::new()));
    registry.register(std::sync::Arc::new(tier1::login::LoginCommand::new()));
    registry.register(std::sync::Arc::new(tier1::logout::LogoutCommand::new()));
    registry.register(std::sync::Arc::new(tier1::resume::ResumeCommand::new()));
    registry.register(std::sync::Arc::new(tier1::diff::DiffCommand::new()));
    registry.register(std::sync::Arc::new(tier1::cost::CostCommand::new()));

    // Tier 2 (stubs)
    registry.register(std::sync::Arc::new(tier2::commit::CommitCommand::new()));
    registry.register(std::sync::Arc::new(tier2::review::ReviewCommand::new()));
    registry.register(std::sync::Arc::new(tier2::memory::MemoryCommand::new()));
    registry.register(std::sync::Arc::new(tier2::skills::SkillsCommand::new()));
    registry.register(std::sync::Arc::new(tier2::tasks::TasksCommand::new()));
    registry.register(std::sync::Arc::new(tier2::mcp::McpCommand::new()));
    registry.register(std::sync::Arc::new(tier2::theme::ThemeCommand::new()));
    registry.register(std::sync::Arc::new(tier2::vim::VimCommand::new()));
    registry.register(std::sync::Arc::new(tier2::context::ContextCommand::new()));

    // Tier 3 (stubs)
    registry.register(std::sync::Arc::new(tier3::doctor::DoctorCommand::new()));
    registry.register(std::sync::Arc::new(tier3::share::ShareCommand::new()));
    registry.register(std::sync::Arc::new(tier3::pr_comments::PrCommentsCommand::new()));
    registry.register(std::sync::Arc::new(tier3::model::ModelCommand::new()));
    registry.register(std::sync::Arc::new(tier3::permissions::PermissionsCommand::new()));
    registry.register(std::sync::Arc::new(tier3::output_style::OutputStyleCommand::new()));
    registry.register(std::sync::Arc::new(tier3::feedback::FeedbackCommand::new()));
    registry.register(std::sync::Arc::new(tier3::hooks::HooksCommand::new()));
    registry.register(std::sync::Arc::new(tier3::effort::EffortCommand::new()));
    registry.register(std::sync::Arc::new(tier3::fast::FastCommand::new()));
    registry.register(std::sync::Arc::new(tier3::brief::BriefCommand::new()));
    registry.register(std::sync::Arc::new(tier3::agents::AgentsCommand::new()));
    registry.register(std::sync::Arc::new(tier3::branch::BranchCommand::new()));
    registry.register(std::sync::Arc::new(tier3::copy::CopyCommand::new()));
    registry.register(std::sync::Arc::new(tier3::exit::ExitCommand::new()));
    registry.register(std::sync::Arc::new(tier3::version::VersionCommand::new()));

    // Utility (stubs)
    registry.register(std::sync::Arc::new(utility::btw::BtwCommand::new()));
    registry.register(std::sync::Arc::new(utility::stats::StatsCommand::new()));
    registry.register(std::sync::Arc::new(utility::status::StatusCommand::new()));
    registry.register(std::sync::Arc::new(utility::files::FilesCommand::new()));
    registry.register(std::sync::Arc::new(utility::export::ExportCommand::new()));
    registry.register(std::sync::Arc::new(utility::rename::RenameCommand::new()));
    registry.register(std::sync::Arc::new(utility::color::ColorCommand::new()));
    registry.register(std::sync::Arc::new(utility::release_notes::ReleaseNotesCommand::new()));
    registry.register(std::sync::Arc::new(utility::keybindings::KeybindingsCommand::new()));
    registry.register(std::sync::Arc::new(utility::passes::PassesCommand::new()));
    registry.register(std::sync::Arc::new(utility::plan::PlanCommand::new()));
    registry.register(std::sync::Arc::new(utility::sandbox_toggle::SandboxToggleCommand::new()));
    registry.register(std::sync::Arc::new(utility::terminal_setup::TerminalSetupCommand::new()));
    registry.register(std::sync::Arc::new(utility::upgrade::UpgradeCommand::new()));
    registry.register(std::sync::Arc::new(utility::usage::UsageCommand::new()));
    registry.register(std::sync::Arc::new(utility::voice::VoiceCommand::new()));
    registry.register(std::sync::Arc::new(utility::chrome::ChromeCommand::new()));
    registry.register(std::sync::Arc::new(utility::ide::IdeCommand::new()));
    registry.register(std::sync::Arc::new(utility::init::InitCommand::new()));
    registry.register(std::sync::Arc::new(utility::remote_setup::RemoteSetupCommand::new()));
    registry.register(std::sync::Arc::new(utility::remote_env::RemoteEnvCommand::new()));
    registry.register(std::sync::Arc::new(utility::privacy_settings::PrivacySettingsCommand::new()));
    registry.register(std::sync::Arc::new(utility::rate_limit_options::RateLimitOptionsCommand::new()));
    registry.register(std::sync::Arc::new(utility::reload_plugins::ReloadPluginsCommand::new()));
    registry.register(std::sync::Arc::new(utility::stickers::StickersCommand::new()));
    registry.register(std::sync::Arc::new(utility::tag::TagCommand::new()));
    registry.register(std::sync::Arc::new(utility::thinkback::ThinkbackCommand::new()));
    registry.register(std::sync::Arc::new(utility::thinkback_play::ThinkbackPlayCommand::new()));
}
