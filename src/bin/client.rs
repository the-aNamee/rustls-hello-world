use std::{io, net::ToSocketAddrs, path::PathBuf, sync::Arc};

use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use serde::Deserialize;
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tokio_rustls::TlsConnector;

const IP: &str = "127.0.0.1:8443";

#[derive(Deserialize)]
struct ClientOptions {
    /// Host
    host: String,
    /// Port
    port: u16,
    /// Domain
    domain: Option<String>,
    /// Cafile
    cafile: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options: ClientOptions = ron::from_str(&fs::read_to_string("client_options.ron").await?)?;

    let _addr = (options.host.as_str(), options.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::ErrorKind::NotFound)
        .expect("There was a problematic problem.");
    let domain = options.domain.unwrap_or(options.host);
    let _content = format!("GET / HTTP/1.0\r\nHost: {}\r\n\r\n", domain);

    let mut root_cert_store = rustls::RootCertStore::empty();
    if let Some(cafile) = &options.cafile {
        for cert in CertificateDer::pem_file_iter(cafile)? {
            root_cert_store.add(cert?)?;
        }
    } else {
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));

    let stream = TcpStream::connect(IP).await?;
    let domain = ServerName::try_from("localhost")?;
    let stream = connector.connect(domain, stream).await?;

    let (reader, mut writer) = tokio::io::split(stream);
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    let reader = BufReader::new(reader);
    let mut server_lines = reader.lines();

    println!("Connected to da server!");

    loop {
        tokio::select! {
            Ok(Some(line)) = stdin.next_line() => {
                writer.write_all(line.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
            Ok(Some(msg)) = server_lines.next_line() => {
                println!("{msg}");
            }
        }
    }

    // Ok(())
}
