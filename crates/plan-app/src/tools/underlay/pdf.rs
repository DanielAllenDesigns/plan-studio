//! Pictures inside PDF files (scanned surveys and existing-house plans).
//!
//! This build has no PDF renderer. A scanned PDF is a page-sized picture, most
//! often a JPEG (`/DCTDecode`) stored whole in the file, and that is what is
//! found here: [`jpeg_pictures`] lists the JPEG streams of a PDF in file order
//! (one per scanned page). A PDF drawn with vector lines and text has none and
//! cannot be used as an underlay (print it to PNG first).

use plan_core::images::image_size;

/// One JPEG picture found in a PDF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfPicture {
    pub width: u32,
    pub height: u32,
    /// The JPEG file bytes, exactly as stored in the PDF.
    pub jpeg: Vec<u8>,
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() || needle.is_empty() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// The JPEG image streams of a PDF, in file order.
pub fn jpeg_pictures(pdf: &[u8]) -> Vec<PdfPicture> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = find(pdf, b"stream", i) {
        // "endstream" contains "stream": skip it.
        if p >= 3 && &pdf[p - 3..p] == b"end" {
            i = p + 6;
            continue;
        }
        let head = &pdf[p.saturating_sub(4096)..p];
        let dict = rfind(head, b"obj").map_or(head, |k| &head[k + 3..]);
        let mut start = p + 6;
        if pdf.get(start) == Some(&b'\r') {
            start += 1;
        }
        if pdf.get(start) == Some(&b'\n') {
            start += 1;
        }
        let Some(end) = find(pdf, b"endstream", start) else {
            break;
        };
        let is_jpeg_image = find(dict, b"/DCTDecode", 0).is_some()
            && find(dict, b"/Image", 0).is_some()
            && pdf[start..end].starts_with(&[0xFF, 0xD8]);
        if is_jpeg_image {
            let mut data = &pdf[start..end];
            while let Some((&last, rest)) = data.split_last() {
                if last == b'\n' || last == b'\r' {
                    data = rest;
                } else {
                    break;
                }
            }
            if let Some((width, height, _)) = image_size(data) {
                out.push(PdfPicture {
                    width,
                    height,
                    jpeg: data.to_vec(),
                });
            }
        }
        i = end + 9;
    }
    out
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const JPG_A: &[u8] = include_bytes!("testdata/rgb444.jpg");
    const JPG_B: &[u8] = include_bytes!("testdata/gray.jpg");

    pub fn sample_pdf() -> Vec<u8> {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        for (n, jpg) in [(1, JPG_A), (2, JPG_B)] {
            pdf.extend_from_slice(
                format!(
                    "{n} 0 obj\n<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\r\n",
                    jpg.len()
                )
                .as_bytes(),
            );
            pdf.extend_from_slice(jpg);
            pdf.extend_from_slice(b"\nendstream\nendobj\n");
        }
        // A Flate image and a content stream are not pictures we can use.
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Subtype /Image /Filter /FlateDecode /Length 4 >>\nstream\nabcd\nendstream\nendobj\n",
        );
        pdf.extend_from_slice(b"4 0 obj\n<< /Length 5 >>\nstream\nq Q\nendstream\nendobj\n%%EOF\n");
        pdf
    }

    #[test]
    fn scanned_pages_are_the_jpeg_streams_in_file_order() {
        let found = jpeg_pictures(&sample_pdf());
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].width, found[0].height), (40, 24));
        assert_eq!((found[1].width, found[1].height), (21, 17));
        assert_eq!(found[0].jpeg, JPG_A);
        assert_eq!(found[1].jpeg, JPG_B);
    }

    #[test]
    fn vector_pdfs_and_junk_hold_no_pictures() {
        assert!(jpeg_pictures(
            b"%PDF-1.4\n1 0 obj\n<< /Length 3 >>\nstream\nabc\nendstream\nendobj\n"
        )
        .is_empty());
        assert!(jpeg_pictures(b"").is_empty());
        assert!(jpeg_pictures(b"stream").is_empty());
    }
}
