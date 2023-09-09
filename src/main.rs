pub mod config;
pub mod event;
mod oauth;
pub mod verses;

use std::{
    io::{stdin, stdout, BufRead, Write},
    sync::Arc,
};

use clap::Parser;
use config::VersesConfig;

use rspotify::{prelude::*, scopes, AuthCodePkceSpotify, Config, Credentials, OAuth};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use verses::Verses;

use crate::oauth::server_oneshot;

async fn prepare_dirs() -> anyhow::Result<()> {
    let cache_dir = home::home_dir().unwrap().join(".cache").join("verses");
    let config_dir = home::home_dir().unwrap().join(".config").join("verses");
    tokio::fs::create_dir_all(config_dir).await?;
    tokio::fs::create_dir_all(cache_dir).await?;
    Ok(())
}

const EXAMPLE_CONFIG: &str = include_str!("./config.example.toml");

/// A TUI spotify synchronized lyrics viewer
#[derive(Parser)]
#[command(about, author, version, long_about = None)]
struct Args {
    /// Whether to just validate the config and exit
    #[arg(long, short)]
    validate: bool,
}

async fn parse_config() -> anyhow::Result<VersesConfig> {
    let config_dir = home::home_dir()
        .unwrap()
        .join(".config")
        .join("verses")
        .join("config.toml");
    if !config_dir.exists() {
        println!("Looks like it's your first time launching Verses!");
        println!("To setup, enter your spotify app client id here.");
        println!("You can create a new app here: https://developer.spotify.com/dashboard/create");
        println!(
            "[IMPORTANT] Make sure to set Redirect URI to \"http://localhost:8888/callback\"!"
        );
        print!("Enter client ID (not client secret!) here: ");
        stdout().lock().flush()?;

        let mut stdin = stdin().lock();
        let mut buf = String::new();
        stdin.read_line(&mut buf)?;
        let buf = &buf[..buf.len() - 1]; // exclude newline

        println!("Saving default configuration...");
        let new_config = EXAMPLE_CONFIG.replace("{{SPOTIFY_CLIENT_ID}}", buf);

        let mut file = tokio::fs::File::create(&config_dir).await?;
        file.write_all(new_config.as_bytes()).await?;
        drop(file);
        println!("Your config has been saved to {config_dir:?}!")
    }

    let mut file = tokio::fs::File::open(&config_dir).await?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).await?;

    VersesConfig::read_from_str(&buf).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create necessary directories first
    prepare_dirs().await?;

    let args = Args::parse();

    // Parsing config
    let verses_config = Arc::new(parse_config().await?);
    if args.validate {
        println!("Config validated");
        return Ok(());
    }

    let creds = Credentials::new_pkce(&verses_config.api.spotify_client_id);
    let oauth = OAuth {
        redirect_uri: "http://localhost:8888/callback".to_string(),
        scopes: scopes!("user-read-playback-state"),
        ..Default::default()
    };

    let config = Config {
        token_cached: true,
        cache_path: home::home_dir()
            .unwrap()
            .join(".cache")
            .join("verses")
            .join("spotify.json"),
        ..Default::default()
    };
    let mut spotify = AuthCodePkceSpotify::with_config(creds.clone(), oauth.clone(), config);
    if let Ok(Some(tk)) = spotify.read_token_cache(true).await {
        *spotify.get_token().lock().await.unwrap() = Some(tk);
        spotify.refresh_token().await?;
    } else {
        let url = spotify.get_authorize_url(None)?;

        match webbrowser::open(&url) {
            Ok(_) => {
                println!("Opened a spotify authentication window in browser.")
            }
            Err(err) => {
                eprintln!("Failed to open a web browser! {err}");
                return Ok(());
            }
        }

        let (code, oauth_state) = server_oneshot().await?;
        if oauth.state != oauth_state {
            println!("Failed to login! Something wrong with OAuth state?");
            return Ok(());
        }
        spotify.request_token(&code).await?;
    }

    let verses = Verses::new(spotify, verses_config);
    verses.run().await?;

    Ok(())
}
