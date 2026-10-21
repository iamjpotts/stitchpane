use std::path::Path;

use cucumber::{given, then, when};
use exif::{Field, In, Rational, Tag, Value};
use image::RgbaImage;
use imaginer_core::codec::{self, EncodeOptions, Format};
use imaginer_core::metadata::ExifGroup;
use imaginer_core::model::ItemId;

use super::{World, close, color, ensure_parent};

/// Writes an image in the format its extension names, and remembers its bytes.
pub fn write_image(world: &mut World, path: &Path, image: &RgbaImage, exif: Option<Vec<u8>>) {
    ensure_parent(path);
    let format = Format::from_path(path).expect("known extension");
    if format == Format::Gif {
        image.save(path).expect("write GIF");
    } else {
        let options = EncodeOptions {
            format,
            quality: 95,
            dpi: None,
            exif,
        };
        codec::write(image, path, &options).expect("write image");
    }
    world
        .snapshots
        .push((path.to_path_buf(), std::fs::read(path).expect("read back")));
    world.images.push(path.to_path_buf());
}

pub fn write_solid(world: &mut World, path: &Path, w: u32, h: u32, rgba: [u8; 4]) {
    write_image(
        world,
        path,
        &RgbaImage::from_pixel(w, h, image::Rgba(rgba)),
        None,
    );
}

pub fn patterned(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([
            (x * 255 / w.max(1)) as u8,
            (y * 255 / h.max(1)) as u8,
            if (x / 4 + y / 4) % 2 == 0 { 40 } else { 220 },
            255,
        ])
    })
}

fn exif_blob(orientation: u16) -> Vec<u8> {
    let ascii = |s: &str| Value::Ascii(vec![s.as_bytes().to_vec()]);
    let fields = vec![
        Field {
            tag: Tag::Make,
            ifd_num: In::PRIMARY,
            value: ascii("TestCam"),
        },
        Field {
            tag: Tag::Model,
            ifd_num: In::PRIMARY,
            value: ascii("T100"),
        },
        Field {
            tag: Tag::Software,
            ifd_num: In::PRIMARY,
            value: ascii("ImaginerTest"),
        },
        Field {
            tag: Tag::Orientation,
            ifd_num: In::PRIMARY,
            value: Value::Short(vec![orientation]),
        },
        Field {
            tag: Tag::ExposureTime,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![Rational { num: 1, denom: 125 }]),
        },
        Field {
            tag: Tag::DateTimeOriginal,
            ifd_num: In::PRIMARY,
            value: ascii("2026:01:02 03:04:05"),
        },
        Field {
            tag: Tag::GPSLatitudeRef,
            ifd_num: In::PRIMARY,
            value: ascii("N"),
        },
        Field {
            tag: Tag::GPSLatitude,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![
                Rational { num: 47, denom: 1 },
                Rational { num: 30, denom: 1 },
                Rational { num: 0, denom: 1 },
            ]),
        },
    ];
    let mut writer = exif::experimental::Writer::new();
    for field in &fields {
        writer.push_field(field);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    writer.write(&mut out, false).expect("write EXIF");
    out.into_inner()
}

#[given(expr = "a {string} image {string} of {int} by {int} pixels")]
fn solid(world: &mut World, name: String, file: String, w: u32, h: u32) {
    let path = world.path(&file);
    write_solid(world, &path, w, h, color(&name));
}

#[given(expr = "a patterned image {string} of {int} by {int} pixels")]
fn pattern(world: &mut World, file: String, w: u32, h: u32) {
    let path = world.path(&file);
    write_image(world, &path, &patterned(w, h), None);
}

#[given(expr = "a noisy image {string} of {int} by {int} pixels")]
fn noisy(world: &mut World, file: String, w: u32, h: u32) {
    let mut seed = 12345u32;
    let image = RgbaImage::from_fn(w, h, |_, _| {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let [a, b, c, _] = seed.to_le_bytes();
        image::Rgba([a, b, c, 255])
    });
    let path = world.path(&file);
    write_image(world, &path, &image, None);
}

#[given(expr = "a photo {string} of {int} by {int} pixels with EXIF")]
fn photo(world: &mut World, file: String, w: u32, h: u32) {
    let path = world.path(&file);
    let image = RgbaImage::from_pixel(w, h, image::Rgba([10, 120, 200, 255]));
    write_image(world, &path, &image, Some(exif_blob(1)));
}

#[given(expr = "a patterned photo {string} of {int} by {int} pixels with EXIF orientation {int}")]
fn turned_photo(world: &mut World, file: String, w: u32, h: u32, orientation: u16) {
    let path = world.path(&file);
    write_image(world, &path, &patterned(w, h), Some(exif_blob(orientation)));
}

fn export(world: &mut World, file: &str, quality: Option<u8>) {
    let path = world.path(file);
    let mut settings = world.editor.export_defaults(&path);
    if let Some(q) = quality {
        settings.quality = q;
    }
    world.error = world.editor.export(&settings).err();
}

#[when(expr = "I export to {string}")]
fn export_default(world: &mut World, file: String) {
    export(world, &file, None);
}

#[when(expr = "I export to {string} at quality {int}")]
fn export_quality(world: &mut World, file: String, quality: u8) {
    export(world, &file, Some(quality));
}

#[when(expr = "I prepare to export to {string}")]
fn prepare_export(world: &mut World, file: String) {
    let path = world.path(&file);
    world.export = Some(world.editor.export_defaults(&path));
}

#[when(expr = "I choose export metadata from image {int} with only the location group")]
fn choose_metadata(world: &mut World, id: u32) {
    let settings = world.export.as_mut().expect("prepared export");
    settings.metadata = Some(ItemId(id));
    settings.groups = vec![ExifGroup::Location];
}

#[when(expr = "I export to {string} with the prepared settings")]
fn export_prepared(world: &mut World, file: String) {
    let mut settings = world.export.clone().expect("prepared export");
    settings.path = world.path(&file);
    settings.format = Format::from_path(&settings.path).expect("format");
    world.editor.export(&settings).expect("export");
}

fn sniff(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "PNG"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "JPEG"
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "WebP"
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        "TIFF"
    } else if bytes.starts_with(b"BM") {
        "BMP"
    } else if bytes.len() > 12 && &bytes[4..8] == b"ftyp" {
        match &bytes[8..12] {
            b"avif" | b"avis" => "AVIF",
            _ => "HEIF",
        }
    } else {
        "unknown"
    }
}

fn decoded(world: &World, file: &str) -> RgbaImage {
    codec::decode(&world.path(file)).expect("decode").pixels
}

#[then(expr = "{string} is a {word} image of {int} by {int} pixels")]
fn is_image(world: &mut World, file: String, format: String, w: u32, h: u32) {
    if let Some(e) = &world.error {
        panic!("export failed: {e}");
    }
    let bytes = std::fs::read(world.path(&file)).expect("read export");
    assert_eq!(sniff(&bytes), format);
    assert_eq!(decoded(world, &file).dimensions(), (w, h));
}

#[then(regex = r#"^"([^"]+)" at (\d+), (\d+) is (roughly )?"([^"]+)"$"#)]
fn pixel_of_file(world: &mut World, file: String, x: u32, y: u32, roughly: String, name: String) {
    if let Some(e) = &world.error {
        panic!("export failed: {e}");
    }
    let pixel = decoded(world, &file).get_pixel(x, y).0;
    let tolerance = if roughly.is_empty() { 12 } else { 48 };
    assert!(
        close(pixel, color(&name), tolerance),
        "{pixel:?} is not {name}"
    );
}

#[then(expr = "{string} is larger than {string}")]
fn larger(world: &mut World, a: String, b: String) {
    let size = |f: &str| std::fs::metadata(world.path(f)).expect("file").len();
    assert!(size(&a) > size(&b), "{} <= {}", size(&a), size(&b));
}

fn recorded_dpi(bytes: &[u8]) -> Option<f64> {
    match sniff(bytes) {
        "PNG" => {
            let at = bytes.windows(4).position(|w| w == b"pHYs")? + 4;
            let ppm = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?);
            (bytes[at + 8] == 1).then(|| f64::from(ppm) * 0.0254)
        }
        "JPEG" => {
            let at = bytes.windows(5).position(|w| w == b"JFIF\0")? + 5;
            let units = bytes[at + 2];
            let density = u16::from_be_bytes([bytes[at + 3], bytes[at + 4]]);
            (units == 1).then_some(f64::from(density))
        }
        "TIFF" => {
            let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(bytes)).ok()?;
            match decoder.get_tag(tiff::tags::Tag::XResolution).ok()? {
                tiff::decoder::ifd::Value::Rational(n, d) => Some(f64::from(n) / f64::from(d)),
                other => other.into_f64().ok(),
            }
        }
        _ => None,
    }
}

#[then(expr = "{string} records {int} DPI")]
fn records_dpi(world: &mut World, file: String, dpi: f64) {
    if let Some(e) = &world.error {
        panic!("export failed: {e}");
    }
    let bytes = std::fs::read(world.path(&file)).expect("read");
    let found = recorded_dpi(&bytes).expect("DPI recorded");
    assert!((found - dpi).abs() < 0.5, "{found} DPI");
}

fn exif_fields(world: &World, file: &str) -> Vec<Field> {
    let file = std::fs::File::open(world.path(file)).expect("open");
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .expect("EXIF present");
    exif.fields().cloned().collect()
}

#[then(expr = "{string} has EXIF {string}")]
fn has_exif(world: &mut World, file: String, tag: String) {
    let fields = exif_fields(world, &file);
    assert!(fields.iter().any(|f| f.tag.to_string() == tag));
}

#[then(expr = "{string} has no EXIF {string}")]
fn lacks_exif(world: &mut World, file: String, tag: String) {
    let fields = exif_fields(world, &file);
    assert!(!fields.iter().any(|f| f.tag.to_string() == tag));
}

#[then(expr = "{string} has EXIF orientation {int} and dimensions {int} by {int}")]
fn exif_geometry(world: &mut World, file: String, orientation: u32, w: u32, h: u32) {
    let fields = exif_fields(world, &file);
    let get = |tag: Tag| {
        fields
            .iter()
            .find(|f| f.tag == tag)
            .and_then(|f| f.value.get_uint(0))
    };
    assert_eq!(get(Tag::Orientation), Some(orientation));
    assert_eq!(get(Tag::PixelXDimension), Some(w));
    assert_eq!(get(Tag::PixelYDimension), Some(h));
}

#[then("GIF is not a writable format")]
fn gif_not_writable(_world: &mut World) {
    assert!(Format::READABLE.contains(&Format::Gif));
    assert!(!Format::WRITABLE.contains(&Format::Gif));
}

#[then(expr = "the export quality is {int}")]
fn export_quality_is(world: &mut World, quality: u8) {
    assert_eq!(world.export.as_ref().expect("prepared").quality, quality);
}

#[then("the export format is lossy")]
fn export_lossy(world: &mut World) {
    assert!(world.export.as_ref().expect("prepared").format.is_lossy());
}

#[then(expr = "the export format is {string}")]
fn export_format(world: &mut World, label: String) {
    assert_eq!(
        world.export.as_ref().expect("prepared").format.label(),
        label
    );
}

#[then(expr = "the export metadata comes from image {int}")]
fn export_metadata_from(world: &mut World, id: u32) {
    assert_eq!(
        world.export.as_ref().expect("prepared").metadata,
        Some(ItemId(id))
    );
}

#[then("the export metadata comes from no image")]
fn export_metadata_none(world: &mut World) {
    assert_eq!(world.export.as_ref().expect("prepared").metadata, None);
}

#[then("the export metadata groups are camera, capture settings, date and time, and other")]
fn export_groups(world: &mut World) {
    assert_eq!(
        world.export.as_ref().expect("prepared").groups,
        vec![
            ExifGroup::Camera,
            ExifGroup::CaptureSettings,
            ExifGroup::DateTime,
            ExifGroup::Other
        ]
    );
}

#[then(expr = "exporting to {string} would overwrite a source")]
fn overwrites(world: &mut World, file: String) {
    assert!(world.editor.export_overwrites_source(&world.path(&file)));
}

#[then(expr = "exporting to {string} would not overwrite a source")]
fn not_overwrites(world: &mut World, file: String) {
    assert!(!world.editor.export_overwrites_source(&world.path(&file)));
}

#[then(expr = "exporting fails with a message containing {string}")]
fn export_fails(world: &mut World, text: String) {
    let error = world.error.as_deref().expect("export should have failed");
    assert!(error.contains(&text), "{error}");
}
