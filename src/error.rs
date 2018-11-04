use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(
        display = "Provided Path does not point to a USB device. Make sure you specify a file matching /dev/disk/by-id/usb-*"
    )]
    NotUSB,

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

    #[fail(display = "Error calling sync")]
    Sync,
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
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
        Error { inner: inner }
    }
}
