use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "Error quering information about the block device")]
    DeviceQuery,

    #[fail(display = "Invalid device name")]
    InvalidDeviceName,

    #[fail(display = "The given block device is neither removable nor a loop device")]
    DangerousDevice,

    #[fail(display = "Partition {} does not exist", _0)]
    NoSuchPartition(u8),

    #[fail(display = "Could not find {}", _0)]
    NoTool(&'static str),

    #[fail(display = "Error creating a temporary directory")]
    TmpDirError,

    #[fail(display = "Partitioning error")]
    Partitioning,

    #[fail(display = "Error formatting filesystems")]
    Formatting,

    #[fail(display = "Error mounting filesystems")]
    Mounting,

    #[fail(display = "Error creating the boot directory")]
    CreateBoot,

    #[fail(display = "Pacstrap error")]
    Pacstrap,

    #[fail(display = "fstab error")]
    Fstab,

    #[fail(display = "Post installation configuration error")]
    PostInstallation,

    #[fail(display = "Initramfs error")]
    Initramfs,

    #[fail(display = "Bootloader error")]
    Bootloader,

    #[fail(display = "Error caused by the interactive mode")]
    Interactive,

    #[fail(display = "Failed umounting filesystems")]
    UmountFailure,

    #[fail(display = "Error setting up an encrypted device")]
    LuksSetup,

    #[fail(display = "Error opening the encrypted device")]
    LuksOpen,

    #[fail(display = "Error closing the encrypted device")]
    LuksClose,

    #[fail(display = "Error detecting whether the root partition is an encrypted device")]
    LuksDetection,

    #[fail(display = "Error setting the locale")]
    Locale,

    #[fail(display = "Failed launching Qemu")]
    Qemu,

    #[fail(display = "Error loading preset \"{}\"", _0)]
    Preset(String),

    #[fail(display = "Missing environment variables \"{:?}\"", _0)]
    MissingEnvironmentVariables(Vec<String>),

    #[fail(display = "Error executing preset script")]
    PresetScript,

    #[fail(display = "Error parsing AUR helper string")]
    AurHelper,

    #[fail(display = "Error creating the image")]
    Image,

    #[fail(display = "Error setting up a loop device: {}", _0)]
    Losetup(String),

    #[fail(display = "Error querying storage devices")]
    StorageDevicesQuery,

    #[fail(display = "There are no removable devices")]
    NoRemovableDevices,
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}
