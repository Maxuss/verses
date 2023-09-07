use anyhow::bail;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub async fn server_oneshot() -> anyhow::Result<(String, String)> {
    let tcp_listener = TcpListener::bind("127.0.0.1:8888").await?;

    // Only accept a single connection
    while let Ok((client, _)) = tcp_listener.accept().await {
        if let Ok(val) = handle_client(client).await {
            return Ok(val);
        }
        // failed to handle client, lets try again
    }

    bail!("Failed to accept OAuth connection!")
}

async fn handle_client(mut client: TcpStream) -> anyhow::Result<(String, String)> {
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

    let data = &String::from_utf8(necessary_info.to_vec())?[19..];
    let code = &data[..276];
    let oauth_state = &data[283..];
    Ok((code.to_owned(), oauth_state.to_owned()))
}
