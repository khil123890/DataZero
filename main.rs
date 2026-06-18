use anyhow::{Context, Result};
use clap::{ArgAction, Parser};
use indicatif::{ProgressBar, ProgressStyle};
use log::*;
use simplelog::*;
use std::path::PathBuf;
use simplelog::ColorChoice;
use std::time::Duration;

mod cert;
mod device;
mod ops;
mod util;

use device::{Device, list_block_devices, detect_hpa};
use ops::{select_method_for_device, do_wipe};
use cert::generate_certificate;

#[derive(Parser, Debug)]
#[command(author, version, about = "DataZero - secure wipe prototype (Rust)")]
struct Args {
    #[arg(long, action = ArgAction::SetTrue)]
    list: bool,
    #[arg(long)]
    target: Option<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    execute: bool,
    #[arg(long)]
    method: Option<String>,
    #[arg(long, default_value = "operator@example.com")]
    operator: String,
    #[arg(long, default_value = "./certs")]
    cert_dir: PathBuf,
}

fn init_logging() -> Result<()> {
    CombinedLogger::init(vec![
        TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
        SimpleLogger::new(LevelFilter::Warn, Config::default()),
    ])
    .context("Failed to init logger")?;
    Ok(())
}

fn show_devices(devs: &[Device]) {
    println!("Detected block devices:");
    for (i, d) in devs.iter().enumerate() {
        println!(
            " [{}] {}  size={} type={} model={:?} serial={:?} tran={:?}",
            i, d.path, d.size, d.media, d.model, d.serial, d.tran
        );
    }
}

fn require_confirmation(dev: &Device, method: &str) -> bool {
    println!("\n*** DANGEROUS OPERATION WARNING ***");
    println!(
        "You are about to perform '{}' on device: {}",
        method, dev.path
    );
    println!("Model: {:?}  Serial: {:?}  Size: {}", dev.model, dev.serial, dev.size);
    let token = dev.serial.clone().unwrap_or_else(|| format!("ERASE-{}", dev.name));
    println!("\nTo confirm, type exactly: {}\n", token);
    use std::io::{stdin, stdout, Write};
    print!("Type to confirm: ");
    stdout().flush().ok();
    let mut typed = String::new();
    stdin().read_line(&mut typed).ok();
    if typed.trim() != token.trim() {
        println!("Confirmation mismatch. Aborting.");
        return false;
    }
    print!("Final confirmation - type 'YES' to proceed: ");
    stdout().flush().ok();
    typed.clear();
    stdin().read_line(&mut typed).ok();
    typed.trim() == "YES"
}

fn main() -> Result<()> {
    init_logging()?;
    let args = Args::parse();

    std::fs::create_dir_all(&args.cert_dir).context("Failed create cert dir")?;

    let devs = list_block_devices().context("Failed to list block devices")?;

    if args.list {
        show_devices(&devs);
        return Ok(());
    }

    let device = if let Some(t) = args.target {
        devs.iter()
            .find(|d| d.path == t)
            .cloned()
            .context("Target not found")?
    } else {
        show_devices(&devs);
        println!("\nSelect device index or 'q' to quit:");
        use std::io::{stdin, stdout, Write};
        print!("Index: ");
        stdout().flush().ok();
        let mut sel = String::new();
        stdin().read_line(&mut sel)?;
        if sel.trim().eq_ignore_ascii_case("q") {
            println!("Aborted.");
            return Ok(());
        }
        let idx: usize = sel.trim().parse().context("Invalid index")?;
        devs.get(idx).cloned().context("Index out of range")?
    };

    if let Some(hpa) = detect_hpa(&device)? {
        println!("HPA/DCO info:\n{}", hpa);
    } else {
        debug!("HPA: not available or hdparm missing");
    }

    let method = args.method.clone().unwrap_or_else(|| select_method_for_device(&device));
    println!("Selected method: {}", method);

    let dry_run = !args.execute;
    if dry_run {
        println!("*** DRY RUN MODE (no destructive commands will be executed). Use --execute to run real wipes. ***");
    } else {
        if !require_confirmation(&device, &method) {
            return Ok(());
        }
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner} {msg}").unwrap());
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_message("Starting wipe orchestration...");

    let result = do_wipe(&device, &method, dry_run)?;

    pb.finish_with_message("Wipe step finished");

    println!("Result: {:?}", result);

    println!("Collecting sample readback SHA256 (non-destructive)...");
    let sample = util::sha256_of_device_sample(&device.path, 1)?;
    println!("Sample SHA256: {}", sample);

    let cert_path = generate_certificate(&args.cert_dir, &device, &method, &result, Some(&args.operator))?;
    println!("Certificate saved to: {}", cert_path.display());

    Ok(())
}
