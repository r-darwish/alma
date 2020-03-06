use super::Tool;
use crate::args;
use crate::error;
use log::debug;

use failure::ResultExt;
use std::os::unix::process::CommandExt as UnixCommandExt;
use std::path::PathBuf;

/// Loads given block device in qemu
/// Uses kvm if it is enabled
pub fn qemu(command: args::QemuCommand) -> Result<(), error::Error> {
    let qemu = Tool::find("qemu-system-x86_64")?;

    let mut run = qemu.execute();
    run.args(&[
        "-m",
        "4G",
        "-netdev",
        "user,id=user.0",
        "-device",
        "virtio-net-pci,netdev=user.0",
        "-device",
        "qemu-xhci,id=xhci",
        "-device",
        "usb-tablet,bus=xhci.0",
        "-drive",
    ])
    .arg(format!(
        "file={},if=virtio,format=raw",
        command.block_device.display()
    ))
    .args(command.args);

    if PathBuf::from("/dev/kvm").exists() {
        debug!("KVM is enabled");
        run.args(&["-enable-kvm", "-cpu", "host"]);
    }

    let err = run.exec();

    Err(err).context(error::ErrorKind::Qemu)?
}
