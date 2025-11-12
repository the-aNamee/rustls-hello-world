use std::{
    io::{self, Error as IoError},
    net::ToSocketAddrs,
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde::Deserialize;
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::broadcast,
};
use tokio_rustls::TlsAcceptor;

#[derive(Deserialize)]
struct ServerOptions {
    /// bind addr
    addr: String,
    /// cert file
    cert: PathBuf,
    /// key file
    key: PathBuf,
    /// echo mode
    echo_mode: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let server_options: ServerOptions =
        ron::from_str(&fs::read_to_string("server_options.ron").await?)?;

    let addr = server_options
        .addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| IoError::from(io::ErrorKind::AddrNotAvailable))?;
    let certs =
        CertificateDer::pem_file_iter(&server_options.cert)?.collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(&server_options.key)?;
    let _flag_echo = server_options.echo_mode;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind(&addr).await?;

    let (tx, _rx) = broadcast::channel::<String>(100);

    println!("Server running. Don't forget to say Beep!");

    loop {
        let (tcp, addr) = listener.accept().await?;
        let acceptor = acceptor.clone();
        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let tls_stream = match acceptor.accept(tcp).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("TLS error: {e}");
                    return;
                }
            };

            let (reader, mut writer) = tokio::io::split(tls_stream);
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            tx.send(format!("{addr} joined")).unwrap();

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.unwrap_or(0) == 0 {
                            println!("{addr} disconnected");
                            break;
                        }
                        let msg = format!("{addr}: {}", line.trim());
                        tx.send(msg).unwrap();
                        line.clear();
                    }

                    msg = rx.recv() => {
                        if let Ok(msg) = msg {
                            if writer.write_all(msg.as_bytes()).await.is_err() {
                                break;
                            }
                            writer.write_all(b"\n").await.ok();
                        }
                    }
                }
            }
        });
    }
}
