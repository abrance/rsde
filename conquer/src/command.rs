use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "conquer")]
#[command(version, about = "Conquer - A powerful server management tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the server
    Server {
        /// Server host address
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Server port
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Configuration file path
        #[arg(short, long)]
        config: Option<String>,

        /// Enable verbose output
        #[arg(short, long)]
        verbose: bool,

        #[arg(short = 'D', long, default_value = "true")]
        debug: bool,
    },
}
