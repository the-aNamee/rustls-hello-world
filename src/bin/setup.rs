use anyhow::Result;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    process::Command,
};

#[tokio::main]
async fn main() -> Result<()> {
    if which::which("rustls-cert-gen").is_err() {
        // Step 1: Install rustls-cert-gen (only if not already installed)
        println!("Installing rustls-cert-gen...");
        let status = Command::new("cargo")
            .arg("install")
            .arg("--locked")
            .arg("rustls-cert-gen")
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("Failed to install rustls-cert-gen");
        }
    }

    // Step 2: Create certs directory
    fs::create_dir_all("certs").await?;

    // Step 3: Run rustls-cert-gen
    println!("Generating certificates...");
    let output = Command::new("rustls-cert-gen")
        .arg("--output")
        .arg("certs/")
        .arg("--san")
        .arg("localhost")
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!(
            "rustls-cert-gen failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("{}", String::from_utf8_lossy(&output.stdout));

    summon_file(
        "client_options.ron",
        r#"(
    host: "127.0.0.1",
    port: 8443,
    domain: Some("localhost"),
    cafile: Some("certs/root-ca.pem"),
)"#,
    )
    .await?;

    summon_file(
        "server_options.ron",
        r#"(
    addr: "127.0.0.1:8443",
    cert: "certs/cert.pem",
    key: "certs/cert.key.pem",
    echo_mode: false
)"#,
    )
    .await?;

    Ok(())
}

/// This is simply shorthand to make this script shorter.
async fn summon_file(path: &str, contents: &str) -> Result<()> {
    let mut file = File::create_new(path).await?;
    file.write_all(contents.as_bytes()).await?;

    Ok(())
}
