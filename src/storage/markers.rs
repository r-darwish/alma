use std::path::Path;
// Marker traits
pub trait BlockDevice: std::fmt::Debug {
    fn path(&self) -> &Path;
}

pub trait Origin {}
