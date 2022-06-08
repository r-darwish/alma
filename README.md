# ALMA - Arch Linux Mobile Appliance

**Note**: This project is no longer maintained, and some people have reported that it made their host system unbootable. Use at your own risk.

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

### Using Arch Linux derivatives

Using Arch Linux derivatives, such as Manjaro, isn't supported it ALMA. It may work and may not. Please do not open bugs or feature 
requests if you are not using the official Arch Linux.

## Usage

### Image creation on removable device
``` shell
sudo alma create /dev/disk/by-id/usb-Generic_USB_Flash_Disk-0:0
```

This will wipe the entire disk and create a bootable installation of Arch Linux. You can use either
removable devices or loop devices. As a precaution, ALMA will not wipe non-removable devices.

Not specifying any path will cause ALMA to interactively prompt the user for a removable device.

### Disk encryption

You can enable disk encryption with the `-e` flag:

``` shell
sudo alma create -e  /dev/disk/by-id/usb-Generic_USB_Flash_Disk-0:0
```

You will be prompted to enter and confirm the encryption passphrase during image creation.

### chroot

After the installation is done you can either boot from it immediately or use `arch-chroot` to
perform further customizations before your first boot (e.g. installing wireless device drivers).

You can run `arch-chroot` via ALMA:

``` shell
sudo alma chroot /dev/disk/by-id/usb-Generic_USB_Flash_Disk-0:0
```

### Create raw image and boot in qemu

For development and testing it may be useful to generate and boot the image in qemu.

Creating a 10GiB raw image, with disk encryption:

``` shell
sudo alma create -e --image 10GiB almatest.img
```

If you receive the following error:
```
Error setting up a loop device: losetup: cannot find an unused loop device
```

Check that you are running ALMA with sudo privileges, and reboot if you have installed a kernel update since your last reboot.

Mounting the raw image to a loop device:

``` shell
sudo losetup -f ./almatest.img
```

Check loop device:
``` shell
 sudo losetup -j ./almatest.img
```
```
/dev/loop0: [2070]:6865917 (/path/to/image/almatest.img)
```
Note that your loop device number may differ.

Run qemu via ALMA:
``` shell
sudo alma qemu /dev/loop0
```

This will boot the image in qemu.

## Presets

Reproducing a build can be easily done using a preset file.

Preset files are simple TOML files which contain:
* A list of packages to install: `packages = ["mypackage"]`
* A post-installation script: `script = """ ... """`
* Environment variables required by the preset (e.g. used in the script): `enironment_variables = ["USERNAME"]`
* A list of shared directories `shared_directories = ["subdirectory"]` - where subdirectory would be available at `/shared_dirs/subdirectory/` for use in the script of the preset.

See the presets directory for examples.

Presets are used via the `--presets` argument (multiple preset files or directories may be provided):

``` shell
sudo ALMA_USER=archie alma create /dev/disk/by-id/usb-Generic_USB_Flash_Disk-0:0 --presets ./presets/user.toml ./presets/custom_preset.toml
```

Preset scripts are executed in the same order they are provided.

If a directory is provided, then all files and subdirectories in the directory are recursively crawled in alphanumeric order (all files must be ALMA .toml files). This allows you to use the following structure to compose many scripts in a specific order:

```
.
├── 00-add_user.toml
├── 01-xorg
│   ├── 00-install.toml
│   └── 01-config.toml
└── 02-i3
    ├── 00-install.toml
    └── 01-copy_dotfiles.toml
```

Example preset TOML:

``` toml
packages = ["sudo"]
script = """
set -eux
useradd -m ${ALMA_USER}
passwd ${ALMA_USER}
usermod -G wheel -a ${ALMA_USER}
echo "%wheel ALL=(ALL) ALL" > /etc/sudoers.d/wheel
"""
environment_variables = ["ALMA_USER"]
```

Note that shared directories in the preset scripts are mounted as bind mounts, so they are *not* mounted read-only. Any changes the custom script makes to the shared directory will be carried out in the preset shared directory of the host system, so be sure to copy (not move) files from the shared directories.

### Order of execution

ALMA installs the packages and presets in the following order:

1. All non-AUR packages are installed
2. If AUR packages are present in the toml files, yay (or another
   specified AUR helper) is installed
3. All AUR packages are installed.
4. Preset scripts are executed according to their filenames in
   alphanumeric order.

Note this may mean you have to workaround some package installations if
they depend on preset scripts.

For example, at the moment you cannot install Rust-based AUR packages in
the `aur_packages` array of the Preset TOMLs if you use rustup,
since rustup needs to be given the toolchain to
install first. This can be worked around by carrying out the AUR
package installation inside the preset script itself in these cases.

## Troubleshooting
### mkinitcpio: /etc/mkinitcpio.d/linux.preset: No such file or directory

Ensure you have both the `linux` and `base` packages installed. Note
that only Arch Linux is supported, not Arch Linux derivatives such as
Manjaro.

### Problem opening /dev/... for reading! Error is 123.

Delete all partitions on the disk first (e.g. with gparted) and try
again.

## Similar projects

* [NomadBSD](http://nomadbsd.org/)

## Useful Resources

* [Arch Wiki: Installing Arch Linux on a USB key](https://wiki.archlinux.org/index.php/Install_Arch_Linux_on_a_USB_key)
* [ValleyCat's Arch Linux USB guide](http://valleycat.org/linux/arch-usb.html?i=1)
