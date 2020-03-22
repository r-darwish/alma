# Full System Preset example

This preset installs an example of a fullly usable Arch Linux system.

Note that the installation of the oh-my-zsh and MiniVim config files means the host computer must have internet access during image creation.

## Usage

Provide ALMA the preset directory, specifying the `ALMA_USER` and `TIMEZONE` environment variables:

i.e. for an image to use with qemu:

```bash
ALMA_USER="test" TIMEZONE="Europe/Madrid" sudo -E alma create --presets ../../presets/system_example --image 5GiB image_name.img
```

## What is included
### User
The user given by `ALMA_USER` is created with a home directory and XDG directories, and given passwordless sudo access.

The root password is also set during installation.

### Microcode

Both Intel and AMD microcode is installed (the correct one will be loaded on boot).

### Networking

NetworkManager and dhcpcd are installed.

nm-applet is run on startup.

### Video drivers

AMD, Intel and Nvidia (proprietary) drivers are installed. The correct one should be loaded according to your system.

### Window server

This preset uses Xorg, not Wayland. A Wayland installation could be created by modifying the Xorg and i3 components (for Wayland and sway respectively).

### PulseAudio

PulseAudio is installed along with bluez for bluetooth headsets (use `bluetoothctl` to connect and pair devices).

pavucontrol can be launched with Meta+v to control the volumes and output devices.

### Virtual Terminal

Urxvt is installed and can be launched with Meta+Enter.

### Display Manager

i3 is installed, a sample configuration is included in this preset.

Meta+r can be used to launch programs via dmenu.

i3status is also installed as a status bar, a sample configuration is included in this preset.

### Text editors

vim and gvim are installed, along with the MiniVim configuration.

emacs and nano are also installed.

### Shell

zsh is installed, along with the oh-my-zsh configuration.

### Web browsers

Firefox and Chromium are installed.

lynx and elinks are also installed for use on the CLI.

### File management

thunar is installed, and can be launch with Meta+f.

### Filesystem tools

gparted and ntfs-3g are installed for working with NTFS partitions and resizing partitions.

### SSH

The openssh client is installed.

### git

git is installed.

### Multimedia

mpd is installed for playing music, along with the ncmpcpp and Ario frontends. It is not configured by default (since the music directory is unknown).

mpv is installed.

### KeepassXC

KeepassXC is installed for password databases.

