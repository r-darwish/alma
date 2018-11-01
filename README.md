# ALMA - Arch Linux Mobile Appliance

This tools installs Arch Linux into a USB drive, making it a customized live Arch Linux bootable
drive. It was inspired by [this](http://valleycat.org/linux/arch-usb.html) article. The USB drive
should be bootable both by UEFI and legacy boot.

## Installation

You should build the project using `cargo build`. An AUR package will be provided with the first
stable version.

## Usage

``` shell
sudo alma create /dev/disk/by-id/usb-Generic_USB_Flash_Disk-0:0
```

This will wipe the entire disk and create a bootable installation of Arch Linux. As a precaution,
this tool will refuse to work with drive paths which don't start with `/dev/disk/by-id/usb-`.

After the installation is done you can either boot from it immediately or use `arch-chroot` to
perform further customizations before your first boot.

## What exactly does it do?

This tool doesn't inspire to be a generic installer for Arch Linux. Instead, it does the minimum
steps required to create a bootable USB with a few tweaks.

1. Partition the disk as suggested [here](http://valleycat.org/linux/arch-usb.html). The last
   partition will be formatted as BTRFS
1. Bootstrap the system using `pacstrap -c`. The `-c` flag will use the host's cache instead the
drive's cache, which will speed up things when you create multiple drives. This tool will install
the base system, grub, intel-ucode, NetworkManager and btrfs-progs
1. Generate initramfs without the `autodetect` hook
1. Install GRUB in both legacy and UEFI modes
