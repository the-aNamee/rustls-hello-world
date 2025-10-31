use std::{
    io::{self, Error as IoError},
    net::ToSocketAddrs,
    path::PathBuf,
    sync::Arc,
};

use anyhow::Result;
use argh::FromArgs;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde::Deserialize;
use tokio::{
    fs,
    io::{AsyncWriteExt, copy, sink, split},
    net::TcpListener,
};
use tokio_rustls::TlsAcceptor;

/// Server Options
#[derive(Deserialize)]
struct Options {
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
    let options: Options = ron::from_str(&fs::read_to_string("options.ron").await?)?;
    println!("{}", options.addr);

    let addr = options
        .addr
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| IoError::from(io::ErrorKind::AddrNotAvailable))?;
    let certs = CertificateDer::pem_file_iter(&options.cert)?.collect::<Result<Vec<_>, _>>()?;
    let key = PrivateKeyDer::from_pem_file(&options.key)?;
    let flag_echo = options.echo_mode;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    let acceptor = TlsAcceptor::from(Arc::new(config));

    let listener = TcpListener::bind(&addr).await?;

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();

        let fut = async move {
            let mut stream = acceptor.accept(stream).await?;

            if flag_echo {
                let (mut reader, mut writer) = split(stream);
                let n = copy(&mut reader, &mut writer).await?;
                writer.flush().await?;
                println!("Echo: {} - {}", peer_addr, n);
            } else {
                let mut output = sink();
                stream
                    .write_all(
                        &b"HTTP/1.0 200 ok\r\n\
                                    Connection: close\r\n\
                                    Content-length: 12\r\n\
                                    \r\n\
                                    Hello world!"[..],
                    )
                    .await?;
                stream.shutdown().await?;
                copy(&mut stream, &mut output).await?;
                println!("Hello: {}", peer_addr);
            }

            Ok(()) as io::Result<()>
        };

        tokio::spawn(async move {
            if let Err(err) = fut.await {
                eprintln!("{:?}", err);
            }
        });
    }
}
