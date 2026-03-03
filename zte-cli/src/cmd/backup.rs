use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;

use zte_lib::zcu;

use crate::cmd::ShellArgs;

const BACKUP_DIRS: &[&str] = &["/etc/config/", "/userconfig/", "/data/"];

#[derive(Subcommand)]
pub enum Cmd {
    /// Pull config directories from the device
    Backup {
        /// Output directory
        #[arg(short, long)]
        output: Option<String>,
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
    },
    /// Decrypt a ZTE config.bin file to XML
    Decrypt {
        /// Encrypted config.bin file
        #[arg(short, long)]
        config: String,
        /// Device serial number for key derivation
        #[arg(short, long)]
        serial: Option<String>,
        /// Explicit AES key (ASCII string)
        #[arg(short, long)]
        key: Option<String>,
        /// Output file (default: <config>.xml)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// View a decrypted XML config as a formatted tree
    View {
        /// Decrypted XML config file
        #[arg(short, long)]
        config: String,
        /// Search pattern to filter entries
        #[arg(short, long)]
        search: Option<String>,
        /// Max tree depth (0=unlimited)
        #[arg(short = 'd', long, default_value_t = 0)]
        max_depth: usize,
    },
    /// Re-encrypt XML config and optionally push to device
    Restore {
        /// Decrypted XML config file
        #[arg(short, long)]
        config: String,
        /// AES key for encryption (ASCII string)
        #[arg(short, long)]
        key: String,
        /// Payload type (0=ECB, 1=CBC)
        #[arg(short = 't', long, default_value_t = 0)]
        payload_type: u32,
        /// Output config.bin path
        #[arg(short, long)]
        output: Option<String>,
        /// Push to device after encryption
        #[arg(long)]
        push: bool,
        /// Remote path on device
        #[arg(long, default_value = "/userconfig/config.bin")]
        remote_path: String,
        /// Shell connection args (SSH default, --adb for USB)
        #[command(flatten)]
        shell: ShellArgs,
        /// Required flag to confirm restore
        #[arg(long)]
        confirm: bool,
    },
}

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Backup { output, shell } => run_backup(output, &shell),
        Cmd::Decrypt {
            config,
            serial,
            key,
            output,
        } => run_decrypt(config, serial, key, output),
        Cmd::View {
            config,
            search,
            max_depth,
        } => run_view(config, search, max_depth),
        Cmd::Restore {
            config,
            key,
            payload_type,
            output,
            push,
            remote_path,
            shell,
            confirm,
        } => run_restore(config, key, payload_type, output, push, remote_path, &shell, confirm),
    }
}

fn run_backup(output: Option<String>, shell: &ShellArgs) -> Result<()> {
    let dev = shell.connect()?;
    println!("Waiting for device ({})...", dev.transport_name());
    dev.wait_for_device(15)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let output_dir = output.unwrap_or_else(|| {
        format!(
            "backup_{}",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        )
    });
    fs::create_dir_all(&output_dir)?;
    println!("{} {output_dir}", "Backing up to:".bold());

    let mut total_pulled = 0;
    for remote_dir in BACKUP_DIRS {
        let dir_name = remote_dir.trim_matches('/').replace('/', "_");
        let local_dir = PathBuf::from(&output_dir).join(&dir_name);
        fs::create_dir_all(&local_dir)?;
        println!("  Pulling {}...", remote_dir.cyan());
        match dev.pull(remote_dir, local_dir.to_str().unwrap()) {
            Ok(result) => {
                println!("    {result}");
                total_pulled += 1;
            }
            Err(e) => {
                eprintln!("    {} Could not pull {remote_dir}: {e}", "Warning:".yellow());
            }
        }
    }

    // Also try known config.bin locations
    for config_path in ["/userconfig/config.bin", "/data/config.bin", "/tmp/config.bin"] {
        let dest = PathBuf::from(&output_dir).join(
            PathBuf::from(config_path)
                .file_name()
                .unwrap_or_default(),
        );
        if dev.pull(config_path, dest.to_str().unwrap()).is_ok() {
            println!("  Pulled {} -> {}", config_path.green(), dest.display());
            total_pulled += 1;
        }
    }

    if total_pulled == 0 {
        anyhow::bail!("No files were pulled. Check device connection and permissions.");
    }
    println!("\n{} Files saved to: {output_dir}", "Backup complete.".bold().green());
    Ok(())
}

fn run_decrypt(
    config_file: String,
    serial: Option<String>,
    key: Option<String>,
    output: Option<String>,
) -> Result<()> {
    let data = fs::read(&config_file).with_context(|| format!("Cannot read {config_file}"))?;
    if data.is_empty() {
        anyhow::bail!("Config file is empty.");
    }
    println!("{} {config_file} ({} bytes)", "Config file:".bold(), data.len());

    let header = zcu::read_header(&data)?;
    println!("  Payload type: {}", header.payload_type);
    if !header.signature.is_empty() {
        println!(
            "  Signature: {}",
            String::from_utf8_lossy(&header.signature)
        );
    }
    println!("  Payload offset: {}", header.payload_offset);

    let key_bytes = key.as_ref().map(|k| k.as_bytes());
    println!("Decrypting...");
    let xml_data =
        zcu::decrypt_config(&data, key_bytes, serial.as_deref())?;

    let output_path = output.unwrap_or_else(|| {
        PathBuf::from(&config_file)
            .with_extension("xml")
            .to_string_lossy()
            .to_string()
    });
    fs::write(&output_path, &xml_data)?;
    println!(
        "\n{} Output: {output_path} ({} bytes)",
        "Decrypted successfully.".bold().green(),
        xml_data.len()
    );
    Ok(())
}

fn run_view(config_file: String, search: Option<String>, max_depth: usize) -> Result<()> {
    let data = fs::read(&config_file)?;
    let text = String::from_utf8_lossy(&data);

    if let Some(pattern) = search {
        search_xml(&text, &pattern);
    } else {
        display_tree(&text, max_depth);
    }
    Ok(())
}

fn display_tree(xml_text: &str, max_depth: usize) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml_text);
    let mut depth = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if max_depth == 0 || depth < max_depth {
                    let indent = "  ".repeat(depth);
                    let name_bytes = e.name().as_ref().to_vec();
                    let name = String::from_utf8_lossy(&name_bytes).to_string();
                    let attrs: Vec<String> = e
                        .attributes()
                        .filter_map(|a| {
                            a.ok().map(|attr| {
                                let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&attr.value).to_string();
                                format!("{k}={v}")
                            })
                        })
                        .collect();
                    let attrs_str = if attrs.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", attrs.join(" "))
                    };
                    println!("{indent}{}{attrs_str}", name.cyan());
                }
                depth += 1;
            }
            Ok(Event::Text(e)) => {
                if max_depth == 0 || depth <= max_depth {
                    let text = e.unescape().unwrap_or_default();
                    let text = text.trim();
                    if !text.is_empty() {
                        let indent = "  ".repeat(depth);
                        println!("{indent}= {}", text.green());
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("XML parse error: {e}");
                break;
            }
            _ => {}
        }
    }
}

fn search_xml(xml_text: &str, pattern: &str) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let pattern_lower = pattern.to_lowercase();
    let mut reader = Reader::from_str(xml_text);
    let mut path = Vec::new();
    let mut matches = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                path.push(name.clone());
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| {
                        a.ok().map(|attr| {
                            (
                                String::from_utf8_lossy(attr.key.as_ref()).to_string(),
                                String::from_utf8_lossy(&attr.value).to_string(),
                            )
                        })
                    })
                    .collect();
                let tag_match = name.to_lowercase().contains(&pattern_lower);
                let attr_match = attrs.iter().any(|(k, v)| {
                    k.to_lowercase().contains(&pattern_lower)
                        || v.to_lowercase().contains(&pattern_lower)
                });
                if tag_match || attr_match {
                    let path_str = path.join("/");
                    let attrs_str = attrs
                        .iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    matches.push(format!("  {path_str} {attrs_str}"));
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if !text.is_empty() && text.to_lowercase().contains(&pattern_lower) {
                    let path_str = path.join("/");
                    matches.push(format!("  {path_str} = {}", text.green()));
                }
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    if matches.is_empty() {
        println!("{}", format!("No matches found for: {pattern}").yellow());
    } else {
        println!(
            "{}\n",
            format!("Found {} match(es) for '{}':", matches.len(), pattern.cyan()).bold()
        );
        for m in &matches {
            println!("{m}");
        }
    }
}

fn run_restore(
    config_file: String,
    key: String,
    payload_type: u32,
    output: Option<String>,
    push: bool,
    remote_path: String,
    shell: &ShellArgs,
    confirm: bool,
) -> Result<()> {
    if !confirm {
        anyhow::bail!("--confirm is required for restore operations.");
    }

    let xml_data = fs::read(&config_file)?;

    // Validate XML
    quick_xml::Reader::from_str(&String::from_utf8_lossy(&xml_data));

    let output_path = output.unwrap_or_else(|| "config_restored.bin".to_string());
    println!(
        "{} {config_file} ({} bytes)",
        "Encrypting:".bold(),
        xml_data.len()
    );
    println!("  Payload type: {payload_type}");

    let encrypted = zcu::encrypt_config(&xml_data, key.as_bytes(), payload_type, b"")?;
    fs::write(&output_path, &encrypted)?;
    println!(
        "{} {output_path} ({} bytes)",
        "Encrypted config saved:".bold().green(),
        encrypted.len()
    );

    if push {
        let dev = shell.connect()?;
        println!("Waiting for device ({})...", dev.transport_name());
        dev.wait_for_device(15)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("  Pushing to {}...", remote_path.cyan());
        let result = dev.push(&output_path, &remote_path)?;
        println!("    {result}");
        println!("{}", "Config restored to device.".bold().green());
    }
    Ok(())
}
