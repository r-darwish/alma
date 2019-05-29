use std::path::PathBuf;
use structopt::StructOpt;

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
}

#[derive(StructOpt)]
pub struct CreateCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str))]
    pub block_device: PathBuf,

    /// Additional pacakges to install
    #[structopt(short = "p", long = "extra-packages", value_name = "package")]
    pub extra_packages: Vec<String>,

    /// Enter interactive chroot before unmounting the drive
    #[structopt(short = "i", long = "interactive")]
    pub interactive: bool,

    /// Encrypt the root partition
    #[structopt(short = "e", long = "encrypted-root")]
    pub encrypted_root: bool,
}

#[derive(StructOpt)]
pub struct ChrootCommand {
    /// Path starting with /dev/disk/by-id for the USB drive
    #[structopt(parse(from_os_str))]
    pub block_device: PathBuf,

    /// Open an encrypted root partition
    #[structopt(short = "e", long = "encrypted-root")]
    pub encrypted_root: bool,

    /// Optional command to run
    #[structopt()]
    pub command: Vec<String>,
}
