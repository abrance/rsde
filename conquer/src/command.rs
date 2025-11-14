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
    Player {
        #[arg(short, long)]
        get: Option<u32>, // 获取玩家信息，参数为玩家 ID
        #[arg(short, long)]
        delete: Option<u32>, // 删除玩家，参数为玩家 ID
        #[arg(short, long)]
        list: Option<bool>, // 列出所有玩家
        #[arg(short, long)]
        create: Option<String>, // 创建玩家，参数为玩家名称
    },
}
