use std::{io, net::ToSocketAddrs, path::PathBuf, sync::Arc};

use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use serde::Deserialize;
use tokio::{
    fs,
    io::{AsyncWriteExt, copy, split, stdin, stdout},
    net::TcpStream,
};
use tokio_rustls::TlsConnector;

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

    let addr = (options.host.as_str(), options.port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::ErrorKind::NotFound)
        .expect("There was a problematic problem.");
    let domain = options.domain.unwrap_or(options.host);
    let content = format!("GET / HTTP/1.0\r\nHost: {}\r\n\r\n", domain);

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

    let stream = TcpStream::connect(&addr).await?;

    let (mut stdin, mut stdout) = (stdin(), stdout());

    let domain = ServerName::try_from(domain.as_str())?.to_owned();
    let mut stream = connector.connect(domain, stream).await?;
    stream.write_all(content.as_bytes()).await?;

    let (mut reader, mut writer) = split(stream);

    tokio::select! {
        ret = copy(&mut reader, &mut stdout) => {
            ret?;
        },
        ret = copy(&mut stdin, &mut writer) => {
            ret?;
            writer.shutdown().await?
        }
    }

    Ok(())
}
