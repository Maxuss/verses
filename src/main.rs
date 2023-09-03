pub mod event;
mod oauth;
pub mod verses;

use rspotify::{prelude::*, scopes, AuthCodePkceSpotify, Config, Credentials, OAuth};
use verses::Verses;

use crate::oauth::server_oneshot;

async fn prepare_dirs() -> anyhow::Result<()> {
    let cache_dir = home::home_dir().unwrap().join(".cache").join("verses");
    let config_dir = home::home_dir().unwrap().join(".config").join("verses");
    tokio::fs::create_dir_all(config_dir).await?;
    tokio::fs::create_dir_all(cache_dir).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create necessary directories first
    prepare_dirs().await?;

    let creds = Credentials::new_pkce("b165ecf5a7f24fd095d55e366a93c8ae");
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

    let verses = Verses::new(spotify);
    verses.run().await?;

    Ok(())
}
