use palc::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    version,
    long_about = "komari-monitor-rs is a third-party high-performance monitoring agent for the komari monitoring service.",
    after_long_help = "Configuration is now loaded from a config file. See komari-agent.conf.example for details.\n\nThis Agent is open-sourced on Github, powered by powerful Rust. Love from Komari"
)]
pub struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    pub config: Option<String>,

    /// Dry Run (for testing)
    #[arg(short, long, default_value_t = false)]
    pub dry_run: bool,
}

impl Args {
    pub fn par() -> Self {
        Self::parse()
    }
}
