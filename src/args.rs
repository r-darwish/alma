use byte_unit::Byte;
use std::path::PathBuf;
use structopt::StructOpt;

fn parse_bytes(src: &str) -> Result<Byte, &'static str> {
    Byte::from_string(src).map_err(|_| "Invalid image size")
}

#[derive(StructOpt)]
#[structopt(name = "alma", about = "Arch Linux Mobile Appliance")]
pub struct App {
    /// Verbose output
    #[structopt(short = "v", long = "verbose")]
    pub verbose: bool,

    #[structopt(subcommand)]
    pub cmd: Command,
}

#[derive(StructOpt)]
pub enum Command {
    #[structopt(name = "create", about = "Create a new Arch Linux USB")]
    Create(CreateCommand),

    #[structopt(name = "chroot", about = "Chroot into exiting Live USB")]
    Chroot(ChrootCommand),

    #[structopt(name = "qemu", about = "Boot the USB with Qemu")]
    Qemu(QemuCommand),
}

#[derive(StructOpt)]
pub struct CreateCommand {
    /// Either a path to a removable block device or a nonexiting file if --image is specified
    #[structopt(parse(from_os_str))]
    pub path: Option<PathBuf>,

    /// Additional pacakges to install
    #[structopt(short = "p", long = "extra-packages", value_name = "package")]
    pub extra_packages: Vec<String>,

    /// Enter interactive chroot before unmounting the drive
    #[structopt(short = "i", long = "interactive")]
    pub interactive: bool,

    /// Encrypt the root partition
    #[structopt(short = "e", long = "encrypted-root")]
    pub encrypted_root: bool,

    /// Path to preset files
    #[structopt(long = "presets", value_name = "preset")]
    pub presets: Vec<PathBuf>,

    /// Create an image with a certain size in the given path instead of using an actual block device
    #[structopt(
        long = "image",
        parse(try_from_str = "parse_bytes"),
        value_name = "size",
        requires = "path"
    )]
    pub image: Option<Byte>,

    /// Overwrite existing image files. Use with caution
    #[structopt(long = "overwrite")]
    pub overwrite: bool,
}

#[derive(StructOpt)]
pub struct ChrootCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str))]
    pub block_device: PathBuf,

    /// Optional command to run
    #[structopt()]
    pub command: Vec<String>,
}

#[derive(StructOpt)]
pub struct QemuCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str))]
    pub block_device: PathBuf,

    /// Arguments to pass to qemu
    #[structopt()]
    pub args: Vec<String>,
}
