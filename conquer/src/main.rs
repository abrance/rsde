mod command;

use clap::Parser;
use command::{Cli, Command};
use snafu::{ResultExt, Snafu};
use std::fs::File;
use std::io::Read;
use util::log::{LogConfig, setup};

// 引入 server crate
use server::http_server::{CustomHttpError, HttpServer, ServerConfig};

#[derive(Debug, Snafu)]
enum CustomError {
    #[snafu(display("Failed to open config file {}: {}", filename, source))]
    OpenConfig {
        filename: String,
        source: std::io::Error,
    },

    #[snafu(display("Failed to parse address {}: {}", addr, source))]
    ParseAddr {
        addr: String,
        source: std::net::AddrParseError,
    },

    #[snafu(display("HTTP Server error: {}", source))]
    HttpError { source: CustomHttpError },
}

// 实现从 CustomHttpError 到 CustomError 的转换
impl From<CustomHttpError> for CustomError {
    fn from(error: CustomHttpError) -> Self {
        CustomError::HttpError { source: error }
    }
}

type Result<T, E = CustomError> = std::result::Result<T, E>;

fn read_config_file(filename: &str) -> Result<String> {
    let mut file = File::open(filename).context(OpenConfigSnafu { filename })?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .context(OpenConfigSnafu { filename })?;
    Ok(contents)
}

async fn handle_server_command(
    host: String,
    port: u16,
    config: Option<String>,
    verbose: bool,
    debug: bool,
) -> Result<()> {
    // 设置日志记录器
    let log_config = LogConfig {
        level: if debug {
            "debug".to_string()
        } else {
            "info".to_string()
        },
        file_path: None,
    };
    setup(log_config);

    if verbose {
        println!("Starting server in verbose mode...");
    }
    print!(
        "Binding to address: {}:{}\nDebug: {}\nVerbose: {}\n",
        host, port, debug, verbose
    );

    if let Some(config_path) = config {
        println!("  Config file: {}", config_path);
        match read_config_file(&config_path) {
            Ok(contents) => println!("Config file contents: {}", contents),
            Err(e) => eprintln!("Error reading config: {}", e),
        }
    } else {
        println!("  No config file specified");
    }

    // 构建并运行 HTTP 服务器
    let server_config = ServerConfig {
        host,
        port,
        max_concurrency: 100,
    };
    let server = HttpServer::new(server_config);
    if let Err(e) = server.run().await {
        eprintln!("Server error: {}", e);
        return Err(CustomError::from(e));
    }

    Ok(())
}

use server::player::{InMemoryPlayerRepository, Player, PlayerRepository};

async fn handle_player_command(
    get: Option<u32>,
    delete: Option<u32>,
    list: Option<bool>,
    create: Option<String>,
) -> Result<()> {
    let mut repo = InMemoryPlayerRepository::new();

    let repo_len = repo.length() as u32 + 1;

    // 示例操作
    if let Some(name) = create {
        let new_id = repo_len;
        let player = Player {
            id: new_id,
            name,
            level: 1,
            ex_cnt: 0,
        };
        repo.save_player(&player).await;
        println!("Created player: {:?}", player);
    }

    if let Some(id) = get {
        match repo.get_player(&id).await {
            Some(player) => println!("Found player: {:?}", player),
            None => println!("Player with ID {} not found", id),
        }
    }

    if let Some(id) = delete {
        if repo.delete_player(&id).await {
            println!("Deleted player with ID {}", id);
        } else {
            println!("Player with ID {} not found for deletion", id);
        }
    }

    if let Some(_) = list {
        let players = repo.list_players().await;
        println!("Listing all players:");
        for player in players {
            println!("{:?}", player);
        }
    }

    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Server {
            host,
            port,
            config,
            verbose,
            debug,
        } => handle_server_command(host, port, config, verbose, debug).await?,
        Command::Player {
            get,
            delete,
            list,
            create,
        } => handle_player_command(get, delete, list, create).await?,
    }

    Ok(())
}
