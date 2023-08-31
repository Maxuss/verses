use std::str::FromStr;

use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT},
    Client,
};
use rspotify::{prelude::*, scopes, AuthCodePkceSpotify, Credentials, OAuth};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let creds = Credentials::new_pkce("b165ecf5a7f24fd095d55e366a93c8ae");
    let oauth = OAuth {
        redirect_uri: "http://localhost:8888/callback".to_string(),
        scopes: scopes!("user-read-playback-state"),
        ..Default::default()
    };
    let mut spotify = AuthCodePkceSpotify::new(creds.clone(), oauth.clone());

    let url = spotify.get_authorize_url(None)?;

    spotify.prompt_for_token(&url).await?;

    let history = spotify.current_playback(None, None::<Vec<_>>).await?;

    println!("Response: {history:#?}");

    Ok(())
}
