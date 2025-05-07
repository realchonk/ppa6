use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use anyhow::Result;

use crate::Backend;

/// A USB backend for [`Printer`](crate::Printer).
pub struct FileBackend {
        file: File,
}

impl FileBackend {
	/// Get a list of printer devices connected via usb.
	pub fn list() -> Result<Vec<PathBuf>> {
                Ok(vec![])
	}

	/// Open a USB printing device.
	pub fn open(path: &Path) -> Result<Self> {
                let file = File::open(path)?;
                Ok(Self {
                        file
                })
	}
}

impl Backend for FileBackend {
	fn send(&mut self, buf: &[u8], _timeout: Duration) -> anyhow::Result<()> {
                // TODO: timeout
                self.file.write_all(buf)?;
		Ok(())
	}

	fn recv(&mut self, buf: &mut [u8], _timeout: Duration) -> anyhow::Result<usize> {
                // TODO: timeout
                let mut nr = 0;
                while nr < buf.len() {
                        let n = self.file.read(&mut buf[nr..])?;
                        if n == 0 {
                            break;
                        }
                        nr += n;
                }
		Ok(nr)
	}
}

