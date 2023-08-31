use std::{str::FromStr, time::Duration};

use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT},
    Client, Request,
};
use rspotify::{prelude::*, scopes, AuthCodePkceSpotify, Credentials, OAuth, Token};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

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

    println!("{url}");
    let (code, oauth_state) = server_oneshot().await?;
    if oauth.state != oauth_state {
        println!("Failed to login! Something wrong with OAuth?");
        return Ok(());
    }
    spotify.request_token(&code).await?;

    let history = spotify.current_playback(None, None::<Vec<_>>).await?;

    println!("Response: {history:#?}");

    Ok(())
}

async fn server_oneshot() -> anyhow::Result<(String, String)> {
    let tcp_listener = TcpListener::bind("127.0.0.1:8888").await?;

    // Only accept a single connection
    let (mut client, _) = tcp_listener.accept().await?;

    // 327 bytes are:
    // GET /callback?code=   :: 19 bytes
    // <Auth code>           :: 276 bytes
    // &state=               :: 7 bytes
    // <OAuth state>         :: 16 bytes
    let mut necessary_info = [0u8; 318];

    client.read_exact(&mut necessary_info).await?;
    client
        .write_all(
            br#"HTTP/1.1 200 OK
content-length: 39
content-type: text/plain

Success! You may now close this window."#,
        )
        .await?;

    let data = &String::from_utf8(necessary_info.to_vec()).unwrap()[19..];
    let code = &data[..276];
    let oauth_state = &data[283..];

    Ok((code.to_owned(), oauth_state.to_owned()))
}
