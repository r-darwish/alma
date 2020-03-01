use super::markers::{BlockDevice, Origin};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Partition<'a> {
    path: PathBuf,
    origin: PhantomData<&'a dyn Origin>,
}

impl<'a> Partition<'a> {
    pub fn new<T: Origin + 'a>(path: PathBuf) -> Self {
        Self {
            path,
            origin: PhantomData,
        }
    }
}

impl<'a> BlockDevice for Partition<'a> {
    fn path(&self) -> &Path {
        &self.path
    }
}
