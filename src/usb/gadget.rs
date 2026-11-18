use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

/// Holds all the USB identity information required by the target console
pub struct GadgetConfig {
    pub gadget_name: &'static str, // e.g., "ds3"
    pub vid: &'static str,
    pub pid: &'static str,
    pub bcd_device: &'static str,
    pub bcd_usb: &'static str,
    pub manufacturer: &'static str,
    pub product: &'static str,
}

pub fn setup_configfs(ffs_mount: &str, config: &GadgetConfig) -> Result<()> {
    let gadget_dir = format!("/sys/kernel/config/usb_gadget/{}", config.gadget_name);

    // 1. Create gadget base directory
    if !Path::new(&gadget_dir).exists() {
        fs::create_dir_all(&gadget_dir).context("Failed to create gadget directory")?;
    } else {
        // Unbind if it already exists to reset the state
        let _ = fs::write(format!("{}/UDC", gadget_dir), "\n");
    }

    // 2. Write dynamic identifiers
    fs::write(format!("{}/idVendor", gadget_dir), config.vid)?;
    fs::write(format!("{}/idProduct", gadget_dir), config.pid)?;
    fs::write(format!("{}/bcdDevice", gadget_dir), config.bcd_device)?;
    fs::write(format!("{}/bcdUSB", gadget_dir), config.bcd_usb)?;

    // 3. Write dynamic Strings
    let strings_dir = format!("{}/strings/0x409", gadget_dir);
    fs::create_dir_all(&strings_dir)?;
    fs::write(format!("{}/manufacturer", strings_dir), config.manufacturer)?;
    fs::write(format!("{}/product", strings_dir), config.product)?;

    // 4. Create Configs and FunctionFS hook dynamically
    let config_dir = format!("{}/configs/c.1", gadget_dir);
    fs::create_dir_all(format!("{}/strings/0x409", config_dir))?;
    
    // The function name must match what we pass to the mount command later
    let func_dir = format!("{}/functions/ffs.{}", gadget_dir, config.gadget_name);
    fs::create_dir_all(&func_dir)?;

    // 5. Symlink the function into the config
    let symlink_path = format!("{}/ffs.{}", config_dir, config.gadget_name);
    if !Path::new(&symlink_path).exists() {
        symlink(&func_dir, &symlink_path).context("Failed to create symlink")?;
    }

    // 6. Mount FunctionFS
    if !Path::new(ffs_mount).exists() {
        fs::create_dir_all(ffs_mount)?;
    }

    let mount_check = Command::new("mountpoint").arg("-q").arg(ffs_mount).status()?;
    if !mount_check.success() {
        Command::new("mount")
            // Pass the dynamic gadget_name to match the ffs.name we created above
            .args(["-t", "functionfs", config.gadget_name, ffs_mount])
            .status()
            .context("Failed to mount FunctionFS")?;
    }

    Ok(())
}

pub fn bind_udc(gadget_name: &str) -> Result<()> {
    let gadget_dir = format!("/sys/kernel/config/usb_gadget/{}", gadget_name);
    let mut udc_name = String::new();

    if let Ok(entries) = fs::read_dir("/sys/class/udc") {
        for entry in entries.flatten() {
            udc_name = entry.file_name().to_string_lossy().into_owned();
            break;
        }
    }

    if udc_name.is_empty() {
        anyhow::bail!("No USB Device Controller (UDC) found.");
    }

    fs::write(format!("{}/UDC", gadget_dir), &udc_name)
        .context("Failed to bind UDC - is the port already in use?")?;

    println!("Successfully bound {} to port: {}", gadget_name, udc_name);
    Ok(())
}