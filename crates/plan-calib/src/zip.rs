//! Read-only zip access and the `.calibz` wrapper.
//!
//! A `.calibz` is a zip (deflate or stored) holding a texture pack plus one
//! embedded `.calib` SQLite catalog. [`ZipArchive`] reads the central
//! directory (including zip64) and extracts single entries by streaming;
//! [`CalibZ`] adds the catalog-specific conveniences.

use crate::error::{Error, Result};
use crate::inflate::inflate;
use crate::source::Source;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const SIG_EOCD: u32 = 0x0605_4b50;
const SIG_EOCD64: u32 = 0x0606_4b50;
const SIG_LOC64: u32 = 0x0706_4b50;
const SIG_CDIR: u32 = 0x0201_4b50;
const SIG_LOCAL: u32 = 0x0403_4b50;

/// One file inside a zip archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// Full path inside the archive.
    pub name: String,
    /// Compression method: 0 = stored, 8 = deflate.
    pub method: u16,
    /// Bytes occupied in the archive.
    pub compressed_size: u64,
    /// Bytes after decompression.
    pub uncompressed_size: u64,
    /// CRC-32 of the uncompressed data.
    pub crc32: u32,
    /// Offset of the entry's local header.
    pub local_header_offset: u64,
}

impl ZipEntry {
    /// The last path component.
    pub fn basename(&self) -> &str {
        self.name.rsplit('/').next().unwrap_or(&self.name)
    }

    /// True for directory entries.
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

/// A zip archive opened for reading.
pub struct ZipArchive {
    src: Source,
    entries: Vec<ZipEntry>,
    by_basename: HashMap<String, usize>,
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"))
}

impl ZipArchive {
    /// Opens `path` and reads its central directory.
    pub fn open(path: impl AsRef<Path>) -> Result<ZipArchive> {
        let src = Source::open(path.as_ref())?;
        let len = src.len();
        if len < 22 {
            return Err(Error::corrupt("file too small to be a zip"));
        }

        // End-of-central-directory record: scan the tail for its signature.
        let tail_len = len.min(22 + 65535) as usize;
        let tail_start = len - tail_len as u64;
        let mut tail = vec![0u8; tail_len];
        src.read_exact_at(&mut tail, tail_start)?;
        let eocd = (0..=tail_len - 22)
            .rev()
            .find(|&i| {
                le32(&tail, i) == SIG_EOCD && i + 22 + le16(&tail, i + 20) as usize == tail_len
            })
            .or_else(|| {
                (0..=tail_len - 22)
                    .rev()
                    .find(|&i| le32(&tail, i) == SIG_EOCD)
            })
            .ok_or_else(|| Error::corrupt("zip end-of-central-directory record not found"))?;
        let mut total = le16(&tail, eocd + 10) as u64;
        let mut cd_size = le32(&tail, eocd + 12) as u64;
        let mut cd_off = le32(&tail, eocd + 16) as u64;

        // A zip64 locator directly before the EOCD overrides the 32-bit fields
        // (mandatory when they are saturated, harmless otherwise).
        {
            let eocd_abs = tail_start + eocd as u64;
            if eocd_abs >= 20 {
                let mut loc = [0u8; 20];
                src.read_exact_at(&mut loc, eocd_abs - 20)?;
                if le32(&loc, 0) == SIG_LOC64 {
                    let off64 = le64(&loc, 8);
                    let mut rec = [0u8; 56];
                    src.read_exact_at(&mut rec, off64)?;
                    if le32(&rec, 0) != SIG_EOCD64 {
                        return Err(Error::corrupt("bad zip64 end-of-central-directory record"));
                    }
                    total = le64(&rec, 32);
                    cd_size = le64(&rec, 40);
                    cd_off = le64(&rec, 48);
                }
            }
        }
        if cd_off.checked_add(cd_size).is_none_or(|e| e > len) {
            return Err(Error::corrupt("central directory lies outside the file"));
        }
        let mut cd = vec![0u8; cd_size as usize];
        src.read_exact_at(&mut cd, cd_off)?;

        let mut entries = Vec::with_capacity(total.min(1 << 20) as usize);
        let mut p = 0usize;
        while p + 46 <= cd.len() && le32(&cd, p) == SIG_CDIR {
            let method = le16(&cd, p + 10);
            let crc32 = le32(&cd, p + 16);
            let mut csize = le32(&cd, p + 20) as u64;
            let mut usize_ = le32(&cd, p + 24) as u64;
            let nlen = le16(&cd, p + 28) as usize;
            let elen = le16(&cd, p + 30) as usize;
            let clen = le16(&cd, p + 32) as usize;
            let mut lho = le32(&cd, p + 42) as u64;
            let end = p + 46 + nlen + elen + clen;
            if end > cd.len() {
                return Err(Error::corrupt("central directory entry overruns directory"));
            }
            let name = String::from_utf8_lossy(&cd[p + 46..p + 46 + nlen]).into_owned();

            // zip64 extended information: fields appear only for saturated values.
            let extra = &cd[p + 46 + nlen..p + 46 + nlen + elen];
            let mut e = 0usize;
            while e + 4 <= extra.len() {
                let id = le16(extra, e);
                let sz = le16(extra, e + 2) as usize;
                let body = &extra[(e + 4).min(extra.len())..(e + 4 + sz).min(extra.len())];
                if id == 1 {
                    let mut q = 0usize;
                    let mut take = |slot: &mut u64, sat: bool| {
                        if sat && q + 8 <= body.len() {
                            *slot = le64(body, q);
                            q += 8;
                        }
                    };
                    let (su, sc, sl) = (
                        usize_ == 0xffff_ffff,
                        csize == 0xffff_ffff,
                        lho == 0xffff_ffff,
                    );
                    take(&mut usize_, su);
                    take(&mut csize, sc);
                    take(&mut lho, sl);
                }
                e += 4 + sz;
            }

            entries.push(ZipEntry {
                name,
                method,
                compressed_size: csize,
                uncompressed_size: usize_,
                crc32,
                local_header_offset: lho,
            });
            p = end;
        }

        let mut by_basename = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            if !e.is_dir() {
                by_basename
                    .entry(e.basename().to_ascii_lowercase())
                    .or_insert(i);
            }
        }
        Ok(ZipArchive {
            src,
            entries,
            by_basename,
        })
    }

    /// All entries in central-directory order.
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// Finds a file by its last path component, ignoring case. If several
    /// directories hold the same name the first one wins.
    pub fn find_basename(&self, name: &str) -> Option<&ZipEntry> {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
        self.by_basename
            .get(&base.to_ascii_lowercase())
            .map(|&i| &self.entries[i])
    }

    /// Streams the decompressed content of `entry` into `out`, verifying size
    /// and CRC-32. Returns the number of bytes written.
    pub fn extract_to<W: Write>(&self, entry: &ZipEntry, out: W) -> Result<u64> {
        let mut head = [0u8; 30];
        self.src
            .read_exact_at(&mut head, entry.local_header_offset)?;
        if le32(&head, 0) != SIG_LOCAL {
            return Err(Error::corrupt(format!(
                "bad local header for '{}'",
                entry.name
            )));
        }
        if le16(&head, 6) & 1 != 0 {
            return Err(Error::unsupported(format!("'{}' is encrypted", entry.name)));
        }
        let data_start =
            entry.local_header_offset + 30 + le16(&head, 26) as u64 + le16(&head, 28) as u64;
        let reader = self
            .src
            .range(data_start, data_start + entry.compressed_size);
        let mut w = CrcWriter {
            inner: out,
            crc: !0,
            count: 0,
        };
        match entry.method {
            0 => {
                let mut r = reader;
                io::copy(&mut r, &mut w)?;
            }
            8 => {
                inflate(reader, &mut w)?;
            }
            m => {
                return Err(Error::unsupported(format!(
                    "compression method {m} for '{}'",
                    entry.name
                )))
            }
        }
        if w.count != entry.uncompressed_size {
            return Err(Error::corrupt(format!(
                "'{}' decompressed to {} bytes, expected {}",
                entry.name, w.count, entry.uncompressed_size
            )));
        }
        if !w.crc != entry.crc32 {
            return Err(Error::corrupt(format!(
                "CRC-32 mismatch in '{}'",
                entry.name
            )));
        }
        Ok(w.count)
    }

    /// Decompresses `entry` into memory.
    pub fn read(&self, entry: &ZipEntry) -> Result<Vec<u8>> {
        let mut v = Vec::with_capacity(entry.uncompressed_size.min(1 << 30) as usize);
        self.extract_to(entry, &mut v)?;
        Ok(v)
    }
}

/// Writer that tracks CRC-32 and byte count.
struct CrcWriter<W> {
    inner: W,
    crc: u32,
    count: u64,
}

impl<W: Write> Write for CrcWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.crc = crc32_update(self.crc, &buf[..n]);
        self.count += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn crc_table() -> &'static [u32; 256] {
    static T: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *slot = c;
        }
        t
    })
}

/// Running CRC-32 (state is pre-inverted; start from `!0`, finish with `!`).
fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    let t = crc_table();
    for &b in data {
        crc = t[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }
    crc
}

/// CRC-32 (IEEE) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    !crc32_update(!0, data)
}

/// A `.calibz`: a zip with textures and one embedded `.calib` catalog.
pub struct CalibZ {
    zip: ZipArchive,
    path: PathBuf,
    calib: usize,
}

impl CalibZ {
    /// Opens a `.calibz` and locates its embedded catalog.
    pub fn open(path: impl AsRef<Path>) -> Result<CalibZ> {
        let path = path.as_ref().to_owned();
        let zip = ZipArchive::open(&path)?;
        let calib = zip
            .entries()
            .iter()
            .position(|e| !e.is_dir() && e.name.to_ascii_lowercase().ends_with(".calib"))
            .ok_or_else(|| Error::not_found(format!("no .calib inside {}", path.display())))?;
        Ok(CalibZ { zip, path, calib })
    }

    /// Path of the `.calibz` file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every entry (textures and the catalog).
    pub fn entries(&self) -> &[ZipEntry] {
        self.zip.entries()
    }

    /// The embedded catalog entry.
    pub fn calib_entry(&self) -> &ZipEntry {
        &self.zip.entries()[self.calib]
    }

    /// Streams the embedded `.calib` to `dest` (created or truncated).
    /// Returns the bytes written.
    pub fn extract_calib_to(&self, dest: impl AsRef<Path>) -> Result<u64> {
        let dest = dest.as_ref();
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let f = std::fs::File::create(dest)?;
        let mut w = io::BufWriter::with_capacity(1 << 20, f);
        let n = self.zip.extract_to(self.calib_entry(), &mut w)?;
        w.flush()?;
        Ok(n)
    }

    /// Extracts the catalog to `<temp>/plan-studio/` under a name derived from
    /// the file name, CRC and size, and reuses an existing extraction of the
    /// same content. Returns the extracted path.
    pub fn extract_calib_to_temp(&self) -> Result<PathBuf> {
        let e = self.calib_entry();
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "catalog".into());
        let safe: String = stem
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let dir = std::env::temp_dir().join("plan-studio");
        let dest = dir.join(format!(
            "{safe}-{:08x}-{}.calib",
            e.crc32, e.uncompressed_size
        ));
        if std::fs::metadata(&dest).is_ok_and(|m| m.len() == e.uncompressed_size) {
            return Ok(dest);
        }
        std::fs::create_dir_all(&dir)?;
        let part = dir.join(format!(
            "{safe}-{:08x}-{}.part{}",
            e.crc32,
            e.uncompressed_size,
            std::process::id()
        ));
        let result = self.extract_calib_to(&part);
        match result {
            Ok(_) => {
                std::fs::rename(&part, &dest)?;
                Ok(dest)
            }
            Err(err) => {
                let _ = std::fs::remove_file(&part);
                Err(err)
            }
        }
    }

    /// True when a file with this basename (any directory, any case) exists.
    pub fn has_texture(&self, name: &str) -> bool {
        self.zip.find_basename(name).is_some()
    }

    /// Extracts an image (or any file) by basename.
    pub fn texture(&self, name: &str) -> Result<Vec<u8>> {
        let e = self.zip.find_basename(name).ok_or_else(|| {
            Error::not_found(format!("texture '{name}' in {}", self.path.display()))
        })?;
        self.zip.read(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, Fixture};
    use std::process::Command;

    fn have_zip() -> bool {
        Command::new("zip").arg("-v").output().is_ok()
    }

    /// Compressible but non-trivial text that makes `zip` choose dynamic Huffman.
    fn text_payload() -> Vec<u8> {
        let mut s = String::new();
        for i in 0..4000 {
            s.push_str(&format!(
                "line {i}: texture {} has {} bytes\n",
                i % 37,
                i * 13 % 997
            ));
        }
        s.into_bytes()
    }

    fn make_zip(
        name: &str,
        flags: &[&str],
    ) -> Option<(testutil::Scratch, PathBuf, Vec<u8>, Vec<u8>)> {
        if !have_zip() {
            eprintln!("zip CLI missing; skipping");
            return None;
        }
        let dir = testutil::scratch_dir(name);
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("textures")).unwrap();
        let text = text_payload();
        std::fs::write(src.join("textures/Oak Floor.txt"), &text).unwrap();
        let bin: Vec<u8> = (0..70_000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        std::fs::write(src.join("Random.bin"), &bin).unwrap();
        let out = dir.join("pack.zip");
        let mut cmd = Command::new("zip");
        cmd.current_dir(&src)
            .arg("-q")
            .arg("-r")
            .args(flags)
            .arg(&out)
            .arg(".");
        assert!(cmd.status().unwrap().success());
        Some((dir, out, text, bin))
    }

    fn check_archive(path: &Path, text: &[u8], bin: &[u8]) {
        let z = ZipArchive::open(path).unwrap();
        let t = z
            .find_basename("oak floor.TXT")
            .expect("case-insensitive basename");
        assert_eq!(t.basename(), "Oak Floor.txt");
        assert_eq!(z.read(t).unwrap(), text);
        let b = z.find_basename("Random.bin").unwrap();
        assert_eq!(z.read(b).unwrap(), bin);
        assert!(z.find_basename("missing.png").is_none());
    }

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn deflate_archive() {
        let Some((_d, p, text, bin)) = make_zip("zip-deflate", &["-9"]) else {
            return;
        };
        check_archive(&p, &text, &bin);
        let z = ZipArchive::open(&p).unwrap();
        assert_eq!(z.find_basename("Oak Floor.txt").unwrap().method, 8);
    }

    #[test]
    fn stored_archive() {
        let Some((_d, p, text, bin)) = make_zip("zip-stored", &["-0"]) else {
            return;
        };
        check_archive(&p, &text, &bin);
        let z = ZipArchive::open(&p).unwrap();
        assert_eq!(z.find_basename("Random.bin").unwrap().method, 0);
    }

    #[test]
    fn zip64_archive() {
        let Some((_d, p, text, bin)) = make_zip("zip-zip64", &["-fz"]) else {
            return;
        };
        check_archive(&p, &text, &bin);
    }

    #[test]
    fn corrupted_data_fails_crc_or_inflate() {
        let Some((_d, p, _, _)) = make_zip("zip-corrupt", &["-9"]) else {
            return;
        };
        let z = ZipArchive::open(&p).unwrap();
        let e = z.find_basename("Oak Floor.txt").unwrap().clone();
        let mut bytes = std::fs::read(&p).unwrap();
        let at = e.local_header_offset as usize
            + 30
            + e.name.len()
            + 20
            + (e.compressed_size as usize / 2);
        bytes[at] ^= 0xff;
        let bad = p.with_file_name("bad.zip");
        std::fs::write(&bad, bytes).unwrap();
        let z2 = ZipArchive::open(&bad).unwrap();
        assert!(z2.read(z2.find_basename("Oak Floor.txt").unwrap()).is_err());
    }

    #[test]
    fn calibz_wraps_a_catalog() {
        let (Some(fx), true) = (Fixture::chief("zip-calibz"), have_zip()) else {
            return;
        };
        let dir = fx.path.parent().unwrap().to_owned();
        let stage = dir.join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        std::fs::copy(&fx.path, stage.join("Fixture.calib")).unwrap();
        std::fs::write(stage.join("Wood.jpg"), b"\xff\xd8\xff fake jpeg").unwrap();
        let out = dir.join("Fixture.calibz");
        assert!(Command::new("zip")
            .current_dir(&stage)
            .args(["-q", "-9"])
            .arg(&out)
            .args(["Fixture.calib", "Wood.jpg"])
            .status()
            .unwrap()
            .success());

        let cz = CalibZ::open(&out).unwrap();
        assert_eq!(cz.entries().len(), 2);
        assert_eq!(cz.calib_entry().name, "Fixture.calib");
        assert_eq!(cz.texture("wood.JPG").unwrap(), b"\xff\xd8\xff fake jpeg");
        assert!(matches!(cz.texture("nope.png"), Err(Error::NotFound(_))));

        let dest = dir.join("out").join("x.calib");
        let n = cz.extract_calib_to(&dest).unwrap();
        assert_eq!(n, std::fs::metadata(&fx.path).unwrap().len());
        assert_eq!(
            std::fs::read(&dest).unwrap(),
            std::fs::read(&fx.path).unwrap()
        );

        let t1 = cz.extract_calib_to_temp().unwrap();
        let t2 = cz.extract_calib_to_temp().unwrap();
        assert_eq!(t1, t2);
        assert!(t1.starts_with(std::env::temp_dir().join("plan-studio")));
        let _ = std::fs::remove_file(t1);
    }
}
