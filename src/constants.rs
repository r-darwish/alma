pub const BOOT_PARTITION_INDEX: u8 = 1;
pub const ROOT_PARTITION_INDEX: u8 = 3;

pub static JOURNALD_CONF: &str = "
[Journal]
Storage=volatile
SystemMaxUse=16M
";

pub const BASE_PACKAGES: [&str; 8] = [
    "base",
    "linux",
    "linux-firmware",
    "grub",
    "efibootmgr",
    "intel-ucode",
    "networkmanager",
    "broadcom-wl",
];
