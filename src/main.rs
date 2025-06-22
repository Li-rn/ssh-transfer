use anyhow::Result;
use clap::Parser;
use ssh_transfer::{cli::Cli, config::Config, run_transfer};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    println!("remote_path: {:?}", cli.command);
    
    // 显示连接信息
    println!("SSH Transfer Tool v0.1.0");
    println!("Target: {}@{}:{}", cli.username, cli.host, cli.port);
    
    let config = Config::from_cli(&cli)?;

    match run_transfer(config).await {
        Ok(_) => {
            println!("\n✅ Transfer completed successfully!\n");
        }
        Err(e) => {
            eprintln!("\n❌ Transfer failed: {}\n", e);
            std::process::exit(1);
        }
    }

    Ok(())
}