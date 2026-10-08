//! Positioned, read-only file access shared by the SQLite and zip readers.
//!
//! On Unix this uses `FileExt::read_at` so no shared cursor exists and reads
//! need only `&self`. Elsewhere (or with the `seek-io` feature) a mutex-guarded
//! seek + read is used instead.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

#[cfg(any(not(unix), feature = "seek-io"))]
use std::io::{Seek, SeekFrom};
#[cfg(any(not(unix), feature = "seek-io"))]
use std::sync::Mutex;

/// A read-only file with random access.
pub(crate) struct Source {
    #[cfg(all(unix, not(feature = "seek-io")))]
    file: File,
    #[cfg(any(not(unix), feature = "seek-io"))]
    file: Mutex<File>,
    len: u64,
}

impl Source {
    /// Opens `path` read-only.
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            #[cfg(all(unix, not(feature = "seek-io")))]
            file,
            #[cfg(any(not(unix), feature = "seek-io"))]
            file: Mutex::new(file),
            len,
        })
    }

    /// File length in bytes at open time.
    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    /// Reads up to `buf.len()` bytes at `offset`; returns the count read.
    #[cfg(all(unix, not(feature = "seek-io")))]
    pub(crate) fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        std::os::unix::fs::FileExt::read_at(&self.file, buf, offset)
    }

    /// Reads up to `buf.len()` bytes at `offset`; returns the count read.
    #[cfg(any(not(unix), feature = "seek-io"))]
    pub(crate) fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let mut f = self.file.lock().unwrap_or_else(|e| e.into_inner());
        f.seek(SeekFrom::Start(offset))?;
        f.read(buf)
    }

    /// Fills `buf` from `offset` or fails with `UnexpectedEof`.
    pub(crate) fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let mut done = 0;
        while done < buf.len() {
            match self.read_at(&mut buf[done..], offset + done as u64) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => done += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// A `Read` adapter over the byte range `[start, end)`.
    pub(crate) fn range(&self, start: u64, end: u64) -> SourceReader<'_> {
        SourceReader {
            src: self,
            pos: start,
            end,
        }
    }
}

/// Sequential reader over a fixed byte range of a [`Source`].
pub(crate) struct SourceReader<'a> {
    src: &'a Source,
    pos: u64,
    end: u64,
}

impl Read for SourceReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.end || buf.is_empty() {
            return Ok(0);
        }
        let want = buf.len().min((self.end - self.pos) as usize);
        let n = self.src.read_at(&mut buf[..want], self.pos)?;
        self.pos += n as u64;
        Ok(n)
    }
}
