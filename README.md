# ALMA - Arch Linux Mobile Appliance

Almost every live Linux distribution out there is meant for a specific purpose, whether it's data
rescue, privacy, penetration testing or anything else. There are some more generic distributions
but all of them are based on squashfs, meaning that changes don't persist reboots.

ALMA is meant for those who wish to have a **mutable** live environment. It installs Arch
Linux into a USB or an SD card, almost as if it was a hard drive. Some configuration is applied in
order to minimize writes to the USB and making sure the system is bootable on both BIOS and UEFI
systems.

Upgrading your packages is as easy as running `pacman -Syu` (or [Topgrade](https://github.com/r-darwish/topgrade/)) while the system is
booted. This tool also provides an easy chroot command, so you can keep your live environment up to
date without having to boot it. Encrypting the root partition is as easy as providing the `-e` flag

## Installation

You can either build the project using cargo build or install the `alma` package from AUR.

## Usage

``` shell
sudo alma create /dev/disk/by-id/usb-Generic_USB_Flash_Disk-0:0
```

This will wipe the entire disk and create a bootable installation of Arch Linux. You can use either
removable devices or loop devices. As a precaution, ALMA will not wipe non-removable devices.

After the installation is done you can either boot from it immediately or use `arch-chroot` to
perform further customizations before your first boot.

Not specifying any path will cause ALMA to interactively prompt the user for a removable device.

## Presets

Reproducing a build can be easily done using a preset file. Presets file are simple TOML file which
contain a list of packages to install, a post-installation script and environment variables required
by the preset. See the presets directory for examples.

## Similar projects

* [NomadBSD](http://nomadbsd.org/)
