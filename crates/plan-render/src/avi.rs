//! Motion-JPEG AVI writer (RIFF `AVI ` with `MJPG` frames), dependency free.
//!
//! The file is the plain OpenDML-less AVI 1.0 layout every player reads:
//! `hdrl` (main header plus one video stream), a `movi` list with one `00dc`
//! chunk per JPEG frame, and an `idx1` index. [`AviWriter`] streams frames to
//! any `Write + Seek` and patches the sizes and the frame count in
//! [`AviWriter::finish`], so a long walkthrough never sits in memory.
//! [`read_avi`] parses the same layout back (tests, and a sanity check after
//! recording).

use crate::image::Image;
use crate::jpeg::encode_jpeg;
use std::io::{self, Seek, SeekFrom, Write};

/// Largest picture side the AVI headers can describe.
pub const MAX_SIDE: u32 = 16_383;

/// Streams JPEG frames into an AVI.
pub struct AviWriter<W: Write + Seek> {
    out: W,
    width: u32,
    height: u32,
    /// `(offset from the start of the movi fourcc, size)` of each frame.
    index: Vec<(u32, u32)>,
    /// Bytes written inside the movi list after its fourcc.
    movi_len: u32,
    max_frame: u32,
    /// File positions patched in `finish`.
    at: Patches,
}

#[derive(Clone, Copy, Default)]
struct Patches {
    riff_size: u64,
    avih_frames: u64,
    avih_buffer: u64,
    strh_length: u64,
    strh_buffer: u64,
    movi_size: u64,
}

fn put_u32(out: &mut impl Write, v: u32) -> io::Result<()> {
    out.write_all(&v.to_le_bytes())
}

fn put_u16(out: &mut impl Write, v: u16) -> io::Result<()> {
    out.write_all(&v.to_le_bytes())
}

impl<W: Write + Seek> AviWriter<W> {
    /// Starts an AVI of `width` x `height` pixels at `fps` frames per second.
    pub fn new(mut out: W, width: u32, height: u32, fps: f64) -> io::Result<Self> {
        let (width, height) = (width.clamp(1, MAX_SIDE), height.clamp(1, MAX_SIDE));
        // Whole rates are 1 s per frame unit; fractional ones keep three decimals.
        let fps = if fps.is_finite() {
            fps.clamp(1.0, 120.0)
        } else {
            12.0
        };
        let rate = ((fps * 1000.0).round() as u32, 1000);
        let micros = (1_000_000.0 / fps).round() as u32;
        let mut at = Patches::default();

        out.write_all(b"RIFF")?;
        at.riff_size = out.stream_position()?;
        put_u32(&mut out, 0)?;
        out.write_all(b"AVI ")?;

        // hdrl: avih (56) + LIST strl { strh (56), strf (40) }.
        out.write_all(b"LIST")?;
        put_u32(&mut out, 4 + (8 + 56) + (12 + (8 + 56) + (8 + 40)))?;
        out.write_all(b"hdrl")?;

        out.write_all(b"avih")?;
        put_u32(&mut out, 56)?;
        put_u32(&mut out, micros)?; // dwMicroSecPerFrame
        put_u32(&mut out, 0)?; // dwMaxBytesPerSec
        put_u32(&mut out, 0)?; // dwPaddingGranularity
        put_u32(&mut out, 0x10)?; // dwFlags: AVIF_HASINDEX
        at.avih_frames = out.stream_position()?;
        put_u32(&mut out, 0)?; // dwTotalFrames
        put_u32(&mut out, 0)?; // dwInitialFrames
        put_u32(&mut out, 1)?; // dwStreams
        at.avih_buffer = out.stream_position()?;
        put_u32(&mut out, 0)?; // dwSuggestedBufferSize
        put_u32(&mut out, width)?;
        put_u32(&mut out, height)?;
        for _ in 0..4 {
            put_u32(&mut out, 0)?; // dwReserved
        }

        out.write_all(b"LIST")?;
        put_u32(&mut out, 4 + (8 + 56) + (8 + 40))?;
        out.write_all(b"strl")?;

        out.write_all(b"strh")?;
        put_u32(&mut out, 56)?;
        out.write_all(b"vids")?;
        out.write_all(b"MJPG")?;
        put_u32(&mut out, 0)?; // dwFlags
        put_u16(&mut out, 0)?; // wPriority
        put_u16(&mut out, 0)?; // wLanguage
        put_u32(&mut out, 0)?; // dwInitialFrames
        put_u32(&mut out, rate.1)?; // dwScale
        put_u32(&mut out, rate.0)?; // dwRate
        put_u32(&mut out, 0)?; // dwStart
        at.strh_length = out.stream_position()?;
        put_u32(&mut out, 0)?; // dwLength
        at.strh_buffer = out.stream_position()?;
        put_u32(&mut out, 0)?; // dwSuggestedBufferSize
        put_u32(&mut out, 0xFFFF_FFFF)?; // dwQuality: default
        put_u32(&mut out, 0)?; // dwSampleSize
        put_u16(&mut out, 0)?;
        put_u16(&mut out, 0)?;
        put_u16(&mut out, width as u16)?;
        put_u16(&mut out, height as u16)?;

        out.write_all(b"strf")?;
        put_u32(&mut out, 40)?;
        put_u32(&mut out, 40)?; // biSize
        put_u32(&mut out, width)?;
        put_u32(&mut out, height)?;
        put_u16(&mut out, 1)?; // biPlanes
        put_u16(&mut out, 24)?; // biBitCount
        out.write_all(b"MJPG")?; // biCompression
        put_u32(&mut out, width * height * 3)?; // biSizeImage
        for _ in 0..4 {
            put_u32(&mut out, 0)?; // pels per meter, colours used and important
        }

        out.write_all(b"LIST")?;
        at.movi_size = out.stream_position()?;
        put_u32(&mut out, 0)?;
        out.write_all(b"movi")?;

        Ok(Self {
            out,
            width,
            height,
            index: Vec::new(),
            movi_len: 4,
            max_frame: 0,
            at,
        })
    }

    /// Picture size.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Frames written so far.
    pub fn frames(&self) -> usize {
        self.index.len()
    }

    /// Adds one already-encoded baseline JPEG frame.
    pub fn add_jpeg(&mut self, jpeg: &[u8]) -> io::Result<()> {
        let size = u32::try_from(jpeg.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too large"))?;
        let padded = size + (size & 1);
        // The AVI is a 32 bit file: stay clear of 4 GiB.
        if u64::from(self.movi_len) + u64::from(padded) + 8 + (self.index.len() as u64 + 1) * 16
            > 0xF000_0000
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the video is larger than 3.5 GB; lower the size or the frame rate",
            ));
        }
        self.out.write_all(b"00dc")?;
        put_u32(&mut self.out, size)?;
        self.out.write_all(jpeg)?;
        if size & 1 == 1 {
            self.out.write_all(&[0])?;
        }
        self.index.push((self.movi_len, size));
        self.movi_len += 8 + padded;
        self.max_frame = self.max_frame.max(size);
        Ok(())
    }

    /// Encodes `image` as a JPEG frame (`quality` 1..=100) and adds it.
    /// Pictures of another size are refused.
    pub fn add_image(&mut self, image: &Image, quality: u8) -> io::Result<()> {
        if (image.width, image.height) != (self.width, self.height) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "frame size differs from the video size",
            ));
        }
        self.add_jpeg(&encode_jpeg(
            image.width,
            image.height,
            &image.rgba,
            quality,
        ))
    }

    /// Writes the index, patches the headers and returns the output.
    pub fn finish(mut self) -> io::Result<W> {
        self.out.write_all(b"idx1")?;
        put_u32(&mut self.out, self.index.len() as u32 * 16)?;
        for &(offset, size) in &self.index {
            self.out.write_all(b"00dc")?;
            put_u32(&mut self.out, 0x10)?; // AVIIF_KEYFRAME
            put_u32(&mut self.out, offset)?;
            put_u32(&mut self.out, size)?;
        }
        let end = self.out.stream_position()?;
        let frames = self.index.len() as u32;
        let patch = |out: &mut W, at: u64, v: u32| -> io::Result<()> {
            out.seek(SeekFrom::Start(at))?;
            put_u32(out, v)
        };
        patch(&mut self.out, self.at.riff_size, (end - 8) as u32)?;
        patch(&mut self.out, self.at.avih_frames, frames)?;
        patch(&mut self.out, self.at.avih_buffer, self.max_frame)?;
        patch(&mut self.out, self.at.strh_length, frames)?;
        patch(&mut self.out, self.at.strh_buffer, self.max_frame)?;
        patch(&mut self.out, self.at.movi_size, self.movi_len)?;
        self.out.seek(SeekFrom::Start(end))?;
        self.out.flush()?;
        Ok(self.out)
    }
}

/// What [`read_avi`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct AviInfo {
    pub width: u32,
    pub height: u32,
    /// Frames per second.
    pub fps: f64,
    /// The stream's codec fourcc (`MJPG`).
    pub codec: [u8; 4],
    /// `(start, length)` of each frame's bytes in the file.
    pub frames: Vec<(usize, usize)>,
}

impl AviInfo {
    /// The bytes of frame `i` in `file`.
    pub fn frame<'a>(&self, file: &'a [u8], i: usize) -> Option<&'a [u8]> {
        let (start, len) = *self.frames.get(i)?;
        file.get(start..start + len)
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Parses an AVI written by [`AviWriter`] (any single-stream AVI whose frames
/// sit in `00dc`/`00db` chunks of a `movi` list).
pub fn read_avi(file: &[u8]) -> Result<AviInfo, String> {
    if file.get(0..4) != Some(b"RIFF") || file.get(8..12) != Some(b"AVI ") {
        return Err("not an AVI file".into());
    }
    let riff_end = (u32_at(file, 4).ok_or("short file")? as usize + 8).min(file.len());
    let mut info = AviInfo {
        width: 0,
        height: 0,
        fps: 0.0,
        codec: [0; 4],
        frames: Vec::new(),
    };
    let mut pos = 12;
    while pos + 8 <= riff_end {
        let id = &file[pos..pos + 4];
        let size = u32_at(file, pos + 4).ok_or("short chunk")? as usize;
        let body = pos + 8;
        if body + size > riff_end {
            return Err("a chunk runs past the end of the file".into());
        }
        if id == b"LIST" {
            let kind = file.get(body..body + 4).ok_or("short list")?;
            match kind {
                b"hdrl" => parse_hdrl(&file[body + 4..body + size], &mut info)?,
                b"movi" => parse_movi(file, body + 4, body + size, &mut info),
                _ => {}
            }
        }
        pos = body + size + (size & 1);
    }
    if info.width == 0 || info.codec == [0; 4] {
        return Err("no video stream".into());
    }
    Ok(info)
}

fn parse_hdrl(list: &[u8], info: &mut AviInfo) -> Result<(), String> {
    let mut pos = 0;
    while pos + 8 <= list.len() {
        let id = &list[pos..pos + 4];
        let size = u32_at(list, pos + 4).ok_or("short header")? as usize;
        let body = pos + 8;
        let end = (body + size).min(list.len());
        match id {
            b"avih" => {
                info.width = u32_at(list, body + 32).ok_or("short avih")?;
                info.height = u32_at(list, body + 36).ok_or("short avih")?;
            }
            b"LIST" => {
                // strl: strh then strf.
                let mut p = body + 4;
                while p + 8 <= end {
                    let sid = &list[p..p + 4];
                    let ssize = u32_at(list, p + 4).ok_or("short stream header")? as usize;
                    if sid == b"strh" {
                        info.codec = list
                            .get(p + 12..p + 16)
                            .and_then(|c| c.try_into().ok())
                            .ok_or("short strh")?;
                        let scale = u32_at(list, p + 28).ok_or("short strh")?;
                        let rate = u32_at(list, p + 32).ok_or("short strh")?;
                        info.fps = f64::from(rate) / f64::from(scale.max(1));
                    }
                    p += 8 + ssize + (ssize & 1);
                }
            }
            _ => {}
        }
        pos = body + size + (size & 1);
    }
    Ok(())
}

fn parse_movi(file: &[u8], mut pos: usize, end: usize, info: &mut AviInfo) {
    while pos + 8 <= end {
        let id = &file[pos..pos + 4];
        let size = u32_at(file, pos + 4).unwrap_or(0) as usize;
        let body = pos + 8;
        if body + size > end {
            break;
        }
        if id == b"00dc" || id == b"00db" {
            info.frames.push((body, size));
        }
        pos = body + size + (size & 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_library::image::jpeg;
    use std::io::Cursor;

    /// A frame whose colour depends on `i`, so frames are told apart.
    fn frame(w: u32, h: u32, i: u32) -> Image {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[
                    (i * 40 % 256) as u8,
                    (x * 255 / w) as u8,
                    (y * 255 / h) as u8,
                    255,
                ]);
            }
        }
        Image {
            width: w,
            height: h,
            rgba,
            hdr: Vec::new(),
        }
    }

    #[test]
    fn frames_round_trip_through_the_avi_and_our_jpeg_decoder() {
        let (w, h) = (45_u32, 30_u32);
        let mut avi = AviWriter::new(Cursor::new(Vec::new()), w, h, 12.0).unwrap();
        for i in 0..5 {
            avi.add_image(&frame(w, h, i), 90).unwrap();
        }
        assert_eq!(avi.frames(), 5);
        let bytes = avi.finish().unwrap().into_inner();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"AVI ");
        assert_eq!(u32_at(&bytes, 4).unwrap() as usize, bytes.len() - 8);
        let info = read_avi(&bytes).unwrap();
        assert_eq!((info.width, info.height), (w, h));
        assert_eq!(&info.codec, b"MJPG");
        assert!((info.fps - 12.0).abs() < 1e-9);
        assert_eq!(info.frames.len(), 5);
        for i in 0..5 {
            let jpg = info.frame(&bytes, i).unwrap();
            assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
            let back = jpeg::decode(jpg).expect("a frame decodes");
            assert_eq!((back.width, back.height), (w, h));
            // The red channel carries the frame number.
            let want = (i as u32 * 40 % 256) as i32;
            let got = i32::from(back.rgba[(15 * w as usize + 20) * 4]);
            assert!((want - got).abs() <= 8, "frame {i}: {want} vs {got}");
        }
    }

    #[test]
    fn the_index_points_at_every_frame() {
        let mut avi = AviWriter::new(Cursor::new(Vec::new()), 16, 16, 24.0).unwrap();
        for i in 0..3 {
            // An odd-sized fake frame checks the pad byte.
            let mut f = vec![0xFF, 0xD8];
            f.extend(std::iter::repeat_n(i as u8, 7 + i));
            f.extend_from_slice(&[0xFF, 0xD9]);
            avi.add_jpeg(&f).unwrap();
        }
        let bytes = avi.finish().unwrap().into_inner();
        let info = read_avi(&bytes).unwrap();
        let movi = bytes.windows(4).position(|w| w == b"movi").unwrap();
        let idx = bytes.windows(4).rposition(|w| w == b"idx1").unwrap();
        assert_eq!(u32_at(&bytes, idx + 4), Some(48));
        for (i, &(start, len)) in info.frames.iter().enumerate() {
            let e = idx + 8 + i * 16;
            assert_eq!(&bytes[e..e + 4], b"00dc");
            assert_eq!(u32_at(&bytes, e + 4), Some(0x10));
            assert_eq!(u32_at(&bytes, e + 8).unwrap() as usize + movi, start - 8);
            assert_eq!(u32_at(&bytes, e + 12).unwrap() as usize, len);
        }
        assert_eq!(info.fps, 24.0);
    }

    #[test]
    fn other_sizes_and_garbage_are_refused() {
        let mut avi = AviWriter::new(Cursor::new(Vec::new()), 16, 16, 12.0).unwrap();
        assert!(avi.add_image(&frame(8, 8, 0), 80).is_err());
        assert!(read_avi(b"RIFFxxxxWAVE").is_err());
        assert!(read_avi(&[]).is_err());
        // An empty video is still a valid, readable file.
        let bytes = avi.finish().unwrap().into_inner();
        assert_eq!(read_avi(&bytes).unwrap().frames.len(), 0);
    }

    #[test]
    fn fractional_rates_are_kept() {
        let avi = AviWriter::new(Cursor::new(Vec::new()), 8, 8, 29.97).unwrap();
        let bytes = avi.finish().unwrap().into_inner();
        assert!((read_avi(&bytes).unwrap().fps - 29.97).abs() < 1e-6);
    }
}
