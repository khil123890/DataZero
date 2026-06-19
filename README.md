# DataZero
Secure data wiping for trustworthy IT asset recycling.

DataZero is a Rust-based CLI application designed to securely erase block devices across multiple platforms (Linux, Windows, and Android via ADB). Built to align with NIST 800-88 sanitization guidelines, the tool safely orchestrates wiping operations, verifies data destruction, and generates cryptographically signed certificates of erasure for compliance and auditing.

# 🚀 Key Features
- Multi-Platform Device Enumeration: Automatically detects HDDs, SSDs, NVMe drives, USBs, eMMC, and UFS devices across native OS environments and connected Android devices.

- Cryptographic Wipe Certificates: Generates an ECDSA P-256 signed JSON certificate detailing the device, operator, wipe method, and timestamp, guaranteeing the integrity of the audit log.

- Post-Wipe Verification: Samples the first 1 MiB of the wiped device to generate a SHA-256 hash, ensuring the initial sectors are successfully cleared.

- HPA/DCO Detection: Integrates with hdparm to detect Hidden Protected Areas (HPA) or Device Configuration Overlays (DCO) that might hide data from standard OS-level wipes.

- Safety First: Requires explicit, token-based user confirmation and operates in a "dry-run" mode by default to prevent accidental data loss.

# 🛠️ Prerequisites
Before building or running DataZero, ensure you have the following installed on your system:

- Rust & Cargo: Install Rust (edition 2021 or later).

- OpenSSL: Required for ECDSA certificate generation.

- ADB (Android Debug Bridge): Optional. Required only if you plan to wipe connected Android devices.

- hdparm: Optional. Required on Linux for detecting HPA/DCO sectors.

# 📦 Installation & Build
Clone the repository and build the project using Cargo:

```Bash:
git clone https://github.com/khil123890/DataZero.git
cd DataZero
cargo build --release
```
The compiled binary will be available in the target/release/ directory.

# 💻 Usage & Commands
DataZero is designed with strict safety rails. If you run the program without the --execute flag, it will run in a safe Dry Run mode and will not perform any destructive operations.

# 1. List Available Devices
To see all local and connected block devices (including Android devices via ADB):

```Bash:
cargo run --release -- --list
```
# 2. Interactive Dry Run
To test the flow, select a device, and see the generated certificate without actually wiping the drive:

```Bash:
cargo run --release
```
# 3. Execute a Wipe on a Specific Target
To permanently erase a device, you must pass the --execute flag and specify the target path (e.g., /dev/sdb, \\.\PHYSICALDRIVE1, or an adb: path).
Note: You will be prompted to type the device serial number and a final "YES" to confirm.

```Bash:
cargo run --release -- --target /dev/sdb --execute
```
# 4. Specify Wipe Method, Operator, and Certificate Directory
You can explicitly define the wipe method, the operator's email/ID, and where the final JSON certificate should be saved.

```Bash:
cargo run --release -- --target /dev/nvme0n1 --execute --method "NIST-800-88-Purge" --operator "admin@example.com" --cert_dir "./audit_logs"
```

# 🏗️ Project Architecture

- `--main.rs`: 
The entry point. Handles CLI argument parsing via clap, logging setup, dry-run safety logic, and user confirmation prompts.

- `--device.rs`: 
Handles the complex enumeration of block devices. It uses OS-specific commands (lsblk on Unix, wmic on Windows) and adb shell commands to identify media types, serial numbers, and device paths.

- `--ops.rs & cert.rs`:
Manages the generation of ECDSA P-256 public/private keys and constructs the cryptographically signed wipe_cert.json to prove the sanitization event occurred.

- `--util.rs`: 
Contains the non-destructive SHA-256 verification logic, reading the initial megabyte of a drive (locally or via adb exec-out dd) to confirm the wipe.
```

# ⚠️ Disclaimer & Warning
THIS TOOL DESTROYS DATA PERMANENTLY.
By using the --execute flag, you are initiating irreversible data erasure.
The authors and contributors of DataZero are not responsible for accidental data loss, bricked devices, or
hardware damage. Always verify your target device path twice before confirming a wipe.
```
