use std::path::Path;

pub trait BlockDevice: std::fmt::Debug {
    fn path(&self) -> &Path;
}

pub trait Origin {}
