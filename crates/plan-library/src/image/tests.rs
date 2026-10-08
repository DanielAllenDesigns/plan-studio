use super::*;

fn td(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/image/testdata")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn check_png(png: &str, rgba: &str) {
    let img = decode(&td(png)).unwrap_or_else(|e| panic!("{png}: {e}"));
    let want = td(rgba);
    assert_eq!((img.width, img.height), (19, 13), "{png}");
    assert_eq!(img.rgba, want, "{png}");
}

#[test]
fn png_color_types_depths_filters_and_interlace() {
    // zlib-compressed (dynamic Huffman) 8-bit RGB.
    check_png("rgb8.png", "rgb8.rgba");
    // Adam7 RGBA with every filter type.
    check_png("rgba_adam7.png", "rgba_adam7.rgba");
    // 16-bit grey keeps the high byte; filters cycle.
    check_png("grey16.png", "grey16.rgba");
    // 4-bit palette with a per-entry tRNS.
    check_png("pal4_trns.png", "pal4_trns.rgba");
    // grey + alpha.
    check_png("ga8.png", "ga8.rgba");
    // 2-bit grey with a color-key tRNS.
    check_png("grey2_key.png", "grey2_key.rgba");
}

#[test]
fn png_round_trips_through_the_stored_encoder() {
    let mut img = Rgba8Image::filled(70, 41, [0, 0, 0, 255]);
    for (i, p) in img.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        p.copy_from_slice(&[
            (i * 3) as u8,
            ((i * 7) >> 2) as u8,
            (i % 251) as u8,
            (i % 7 * 30 + 45) as u8,
        ]);
    }
    let bytes = png::encode_rgba(&img);
    assert_eq!(decode(&bytes).unwrap(), img);
}

#[test]
fn bad_png_is_an_error_not_a_panic() {
    let good = td("rgb8.png");
    for cut in [0, 5, 20, 40, good.len() - 20, good.len() - 1] {
        let _ = decode(&good[..cut]);
    }
    let mut bad = good.clone();
    let n = bad.len();
    for b in &mut bad[n / 2..n - 12] {
        *b ^= 0x5A;
    }
    let _ = decode(&bad);
    assert_eq!(decode(b"GIF89a....").unwrap_err(), Error::UnknownFormat);
    // A huge declared size is refused before allocating.
    let mut huge = png::SIGNATURE.to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    png::write_chunk(&mut huge, b"IHDR", &ihdr);
    assert!(matches!(decode(&huge), Err(Error::Corrupt(_))));
}

/// Reads a binary PNM (P5 grey or P6 RGB) into RGB bytes.
fn pnm(name: &str) -> (usize, usize, Vec<u8>) {
    let d = td(name);
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while d[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !d[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8(d[s..i].to_vec()).unwrap());
    }
    i += 1;
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let body = &d[i..];
    let rgb = if fields[0] == "P5" {
        body.iter().flat_map(|&g| [g, g, g]).collect()
    } else {
        body.to_vec()
    };
    (w, h, rgb)
}

fn check_jpeg(jpg: &str, pnm_name: &str) -> (f64, i32) {
    let img = decode(&td(jpg)).unwrap_or_else(|e| panic!("{jpg}: {e}"));
    let (w, h, rgb) = pnm(pnm_name);
    assert_eq!((img.width as usize, img.height as usize), (w, h), "{jpg}");
    let (mut total, mut worst) = (0u64, 0i32);
    for i in 0..w * h {
        for c in 0..3 {
            let d = (i32::from(img.rgba[i * 4 + c]) - i32::from(rgb[i * 3 + c])).abs();
            total += d as u64;
            worst = worst.max(d);
        }
        assert_eq!(img.rgba[i * 4 + 3], 255);
    }
    (total as f64 / (w * h * 3) as f64, worst)
}

#[test]
fn jpeg_matches_libjpeg_for_every_sampling_and_mode() {
    // (file, reference, mean-error limit, worst-error limit). Full-resolution
    // chroma must match libjpeg's IDCT almost exactly; subsampled chroma
    // differs only by the upsampling filter at the sharp edges.
    let cases = [
        ("b444.jpg", "b444.pnm", 0.5, 3),
        ("p444.jpg", "p444.pnm", 0.5, 3),
        ("gray.jpg", "gray.pnm", 0.5, 3),
        ("b420.jpg", "b420.pnm", 2.0, 40),
        ("b422.jpg", "b422.pnm", 2.0, 40),
        ("p420.jpg", "p420.pnm", 2.0, 40),
        ("r420.jpg", "r420.pnm", 2.0, 40),
    ];
    for (jpg, reference, mean_max, worst_max) in cases {
        let (mean, worst) = check_jpeg(jpg, reference);
        assert!(mean <= mean_max, "{jpg}: mean error {mean}");
        assert!(worst <= worst_max, "{jpg}: worst error {worst}");
    }
}

#[test]
fn progressive_and_baseline_decode_to_the_same_picture() {
    let a = decode(&td("b420.jpg")).unwrap();
    let b = decode(&td("p420.jpg")).unwrap();
    assert_eq!((a.width, a.height), (b.width, b.height));
    // Same DCT coefficients, so the pictures are identical.
    assert_eq!(a.rgba, b.rgba);
    // Restart markers do not change the result either.
    assert_eq!(a.rgba, decode(&td("r420.jpg")).unwrap().rgba);
}

#[test]
fn jpeg_dimensions_are_not_rotated_by_exif_orientation() {
    // Insert an APP1/Exif segment with Orientation = 6 (rotate 90 degrees)
    // right after SOI; the decoder ignores it, so the 37 x 29 size and pixels stay.
    let plain = td("b420.jpg");
    let mut exif =
        b"Exif\0\0MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0".to_vec();
    let seg_len = (exif.len() + 2) as u16;
    let mut with = vec![0xFF, 0xD8, 0xFF, 0xE1];
    with.extend_from_slice(&seg_len.to_be_bytes());
    with.append(&mut exif);
    with.extend_from_slice(&plain[2..]);
    let a = decode(&plain).unwrap();
    let b = decode(&with).unwrap();
    assert_eq!((b.width, b.height), (37, 29));
    assert_eq!(a, b);
}

#[test]
fn damaged_jpeg_never_panics() {
    let good = td("p420.jpg");
    for cut in [0, 2, 3, 20, 100, 300, good.len() - 30, good.len() - 2] {
        let _ = decode(&good[..cut]);
    }
    for seed in 0..40u32 {
        let mut bad = good.clone();
        let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(12345);
        for _ in 0..6 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let i = 100 + (x as usize >> 8) % (bad.len() - 100);
            bad[i] ^= (x >> 3) as u8 | 1;
        }
        let _ = decode(&bad);
    }
    assert!(decode(&[0xFF, 0xD8, 0xFF, 0xD9]).is_err());
    assert!(matches!(decode(b"junk"), Err(Error::UnknownFormat)));
}

#[test]
fn the_idct_matches_a_float_reference() {
    // Pseudo-random coefficients, flat quantization table.
    let q = [3u16; 64];
    let mut coef = [0i16; 64];
    let mut x = 7u32;
    for (i, c) in coef.iter_mut().enumerate() {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        *c = if i < 20 {
            ((x >> 16) % 60) as i16 - 30
        } else {
            0
        };
    }
    let mut got = [0u8; 64];
    jpeg_idct_for_test(&coef, &q, &mut got);
    for y in 0..8 {
        for xx in 0..8 {
            let mut s = 0.0f64;
            for v in 0..8 {
                for u in 0..8 {
                    let cu = if u == 0 {
                        std::f64::consts::FRAC_1_SQRT_2
                    } else {
                        1.0
                    };
                    let cv = if v == 0 {
                        std::f64::consts::FRAC_1_SQRT_2
                    } else {
                        1.0
                    };
                    s += cu
                        * cv
                        * f64::from(coef[v * 8 + u])
                        * f64::from(q[v * 8 + u])
                        * (((2 * xx + 1) * u) as f64 * std::f64::consts::PI / 16.0).cos()
                        * (((2 * y + 1) * v) as f64 * std::f64::consts::PI / 16.0).cos();
                }
            }
            let want = (s / 4.0 + 128.0).round().clamp(0.0, 255.0);
            assert!(
                (f64::from(got[y * 8 + xx]) - want).abs() <= 1.0,
                "({xx},{y}) {} vs {want}",
                got[y * 8 + xx]
            );
        }
    }
}

fn jpeg_idct_for_test(coef: &[i16; 64], q: &[u16; 64], out: &mut [u8; 64]) {
    jpeg::idct_block_for_test(coef, q, out);
}

#[test]
fn box_downscale_mipmaps_and_average() {
    let mut img = Rgba8Image::filled(8, 4, [0, 0, 0, 255]);
    for y in 0..4 {
        for x in 4..8 {
            img.rgba[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    let small = img.downscaled(4);
    assert_eq!((small.width, small.height), (4, 2));
    assert_eq!(small.pixel(0, 0), [0, 0, 0, 255]);
    assert_eq!(small.pixel(3, 1), [255, 255, 255, 255]);
    // No change when already small enough.
    assert_eq!(img.downscaled(64), img);
    let chain = mipmaps(&img);
    let dims: Vec<_> = chain.iter().map(|m| (m.width, m.height)).collect();
    assert_eq!(dims, vec![(8, 4), (4, 2), (2, 1), (1, 1)]);
    // Averaging black and white in linear light gives sRGB ~188, not 128.
    let last = chain.last().unwrap().pixel(0, 0);
    assert!((180..=196).contains(&last[0]), "{last:?}");
    assert!(!img.has_alpha());
    assert_eq!(
        Rgba8Image::filled(2, 2, [10, 20, 30, 255]).average_color(),
        [10, 20, 30, 255]
    );
}

#[test]
fn transparent_pixels_do_not_bleed_into_averages() {
    let mut img = Rgba8Image::filled(2, 1, [255, 0, 0, 255]);
    img.rgba[4..8].copy_from_slice(&[0, 255, 0, 0]);
    let one = img.resized_box(1, 1);
    assert_eq!(&one.rgba[..3], &[255, 0, 0]);
    assert!(img.has_alpha());
}

/// Real Chief textures, if this machine has them. Run with
/// `cargo test -p plan-library --release -- --ignored --nocapture`.
#[test]
#[ignore]
fn chief_textures_decode_fast() {
    let dir = std::path::Path::new(
        "/Library/Application Support/Chief Architect Premier X18/Referenced Files",
    );
    let Ok(rd) = std::fs::read_dir(dir) else {
        eprintln!("no Chief install here");
        return;
    };
    if let Ok(path) = std::env::var("PLAN_IMAGE_BENCH") {
        let bytes = std::fs::read(&path).unwrap();
        let t = std::time::Instant::now();
        let img = decode(&bytes).unwrap();
        eprintln!("{path}: {}x{} in {:?}", img.width, img.height, t.elapsed());
    }
    let mut worst = std::time::Duration::ZERO;
    let mut done = 0;
    for e in rd.flatten().take(4000) {
        let p = e.path();
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "jpg" | "jpeg" | "png") || done >= 60 {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let t = std::time::Instant::now();
        match decode(&bytes) {
            Ok(img) => {
                let dt = t.elapsed();
                worst = worst.max(dt);
                done += 1;
                if img.width * img.height >= 1_000_000 {
                    eprintln!(
                        "{:?} {}x{} {:?}",
                        p.file_name().unwrap(),
                        img.width,
                        img.height,
                        dt
                    );
                }
            }
            Err(Error::Unsupported(m)) => {
                eprintln!("{:?}: unsupported ({m})", p.file_name().unwrap())
            }
            Err(e) => panic!("{:?}: {e}", p.file_name().unwrap()),
        }
    }
    eprintln!("decoded {done} Chief textures, slowest {worst:?}");
    assert!(done > 0);
}
