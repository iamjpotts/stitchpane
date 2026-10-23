//! Reading and writing image files (CONV-001 to CONV-003, EXPORT-002, EXPORT-004).
//! HEIF and AVIF go through libheif; everything else through the `image` crate.

use std::fmt;
use std::io::{BufWriter, Cursor};
use std::path::Path;

use image::codecs::jpeg::{JpegEncoder, PixelDensity, PixelDensityUnit};
use image::codecs::png::PngEncoder;
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageReader, RgbImage, RgbaImage};

use crate::panic_free;
use crate::panic_free::Clamped;

/// Image file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Png,
    Jpeg,
    WebP,
    Tiff,
    Bmp,
    Gif,
    Heif,
    Avif,
}

impl Format {
    /// Formats that can be opened (CONV-001).
    pub const READABLE: [Format; 8] = [
        Format::Png,
        Format::Jpeg,
        Format::WebP,
        Format::Tiff,
        Format::Bmp,
        Format::Gif,
        Format::Heif,
        Format::Avif,
    ];

    /// Formats that can be exported (CONV-002).
    pub const WRITABLE: [Format; 7] = [
        Format::Png,
        Format::Jpeg,
        Format::WebP,
        Format::Tiff,
        Format::Bmp,
        Format::Heif,
        Format::Avif,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::WebP => "WebP",
            Format::Tiff => "TIFF",
            Format::Bmp => "BMP",
            Format::Gif => "GIF",
            Format::Heif => "HEIF",
            Format::Avif => "AVIF",
        }
    }

    /// File name extensions, preferred first.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Png => &["png"],
            Format::Jpeg => &["jpg", "jpeg", "jpe"],
            Format::WebP => &["webp"],
            Format::Tiff => &["tif", "tiff"],
            Format::Bmp => &["bmp"],
            Format::Gif => &["gif"],
            Format::Heif => &["heic", "heif", "hif"],
            Format::Avif => &["avif"],
        }
    }

    pub fn from_extension(ext: &str) -> Option<Format> {
        let ext = ext.to_ascii_lowercase();
        Self::READABLE
            .into_iter()
            .find(|f| f.extensions().contains(&ext.as_str()))
    }

    pub fn from_path(path: &Path) -> Option<Format> {
        Self::from_extension(path.extension()?.to_str()?)
    }

    /// Whether export offers a quality setting (CONV-003).
    pub fn is_lossy(self) -> bool {
        matches!(
            self,
            Format::Jpeg | Format::WebP | Format::Heif | Format::Avif
        )
    }

    /// Whether the format keeps transparency (EXPORT-004).
    pub fn supports_alpha(self) -> bool {
        !matches!(self, Format::Jpeg | Format::Bmp)
    }

    /// Whether export writes DPI metadata for this format (EXPORT-002).
    pub fn supports_dpi(self) -> bool {
        matches!(
            self,
            Format::Png | Format::Jpeg | Format::Tiff | Format::Bmp
        )
    }

    /// Whether export writes EXIF metadata for this format (META-005).
    pub fn supports_exif(self) -> bool {
        matches!(
            self,
            Format::Png | Format::Jpeg | Format::WebP | Format::Heif | Format::Avif
        )
    }
}

#[derive(Debug)]
pub enum CodecError {
    Io(std::io::Error),
    UnknownFormat,
    Decode(String),
    Encode(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::Io(e) => e.fmt(f),
            CodecError::UnknownFormat => f.write_str("unrecognized image format"),
            CodecError::Decode(e) => write!(f, "could not read the image: {e}"),
            CodecError::Encode(e) => write!(f, "could not write the image: {e}"),
        }
    }
}

impl std::error::Error for CodecError {}

impl From<std::io::Error> for CodecError {
    fn from(e: std::io::Error) -> Self {
        CodecError::Io(e)
    }
}

/// A decoded image, upright (META-006), with its raw EXIF block if any.
#[derive(Clone, Debug)]
pub struct Decoded {
    pub pixels: RgbaImage,
    /// TIFF-structured EXIF data, without the `Exif\0\0` prefix.
    pub exif: Option<Vec<u8>>,
}

fn is_heif_container(header: &[u8]) -> bool {
    header.len() >= 12 && header.get(4..8) == Some(b"ftyp".as_slice())
}

/// Decodes an image file, applying its EXIF orientation (META-006).
pub fn decode(path: &Path) -> Result<Decoded, CodecError> {
    let bytes = std::fs::read(path)?;
    decode_bytes(&bytes)
}

pub fn decode_bytes(bytes: &[u8]) -> Result<Decoded, CodecError> {
    if is_heif_container(bytes) {
        return heif::decode(bytes);
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(CodecError::Io)?;
    if reader.format().is_none() {
        return Err(CodecError::UnknownFormat);
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| CodecError::Decode(e.to_string()))?;
    // unreadable EXIF is treated as none, so the image still opens
    let exif = match decoder.exif_metadata() {
        Ok(exif) => exif.map(strip_exif_prefix),
        Err(_) => None,
    };
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|e| CodecError::Decode(e.to_string()))?;
    image.apply_orientation(orientation);
    Ok(Decoded {
        pixels: image.to_rgba8(),
        exif,
    })
}

fn strip_exif_prefix(exif: Vec<u8>) -> Vec<u8> {
    match exif.strip_prefix(b"Exif\0\0") {
        Some(rest) => rest.to_vec(),
        None => exif,
    }
}

/// Export settings for one file.
#[derive(Clone, Debug)]
pub struct EncodeOptions {
    pub format: Format,
    /// 1–100, for lossy formats (CONV-003).
    pub quality: u8,
    pub dpi: Option<f64>,
    /// TIFF-structured EXIF data to embed.
    pub exif: Option<Vec<u8>>,
}

/// Default lossy quality (CONV-003).
pub const DEFAULT_QUALITY: u8 = 90;

/// Composites transparent pixels onto white (EXPORT-004).
pub fn flatten_onto_white(image: &RgbaImage) -> Result<RgbImage, CodecError> {
    panic_free::image_from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel_checked(x, y).map_or([0; 4], |p| p.0);
        let a = u32::from(a);
        // a <= 255, so none of this saturates
        let blend = |c: u8| {
            let white = 255u32.saturating_mul(255u32.saturating_sub(a));
            (u32::from(c)
                .saturating_mul(a)
                .saturating_add(white)
                .saturating_add(127)
                / 255) as u8
        };
        image::Rgb([blend(r), blend(g), blend(b)])
    })
    .ok_or_else(|| CodecError::Encode("image too large".into()))
}

/// Encodes an image to bytes in the requested format.
pub fn encode(image: &RgbaImage, options: &EncodeOptions) -> Result<Vec<u8>, CodecError> {
    let err = |e: &dyn fmt::Display| CodecError::Encode(e.to_string());
    let (w, h) = image.dimensions();
    let mut out = Vec::new();
    match options.format {
        Format::Png => {
            let mut encoder = PngEncoder::new(&mut out);
            if let Some(exif) = &options.exif {
                encoder
                    .set_exif_metadata(exif.clone())
                    .map_err(|e| err(&e))?;
            }
            encoder
                .write_image(image.as_raw(), w, h, image::ExtendedColorType::Rgba8)
                .map_err(|e| err(&e))?;
            if let Some(dpi) = options.dpi {
                out = png_with_dpi(&out, dpi).ok_or_else(|| err(&"malformed PNG"))?;
            }
        }
        Format::Jpeg => {
            let rgb = flatten_onto_white(image)?;
            let mut encoder =
                JpegEncoder::new_with_quality(&mut out, options.quality.clamped(1, 100));
            if let Some(dpi) = options.dpi {
                let d = dpi.round().clamped(1.0, f64::from(u16::MAX)) as u16;
                encoder.set_pixel_density(PixelDensity {
                    density: (d, d),
                    unit: PixelDensityUnit::Inches,
                });
            }
            if let Some(exif) = &options.exif {
                encoder
                    .set_exif_metadata(exif.clone())
                    .map_err(|e| err(&e))?;
            }
            encoder
                .write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map_err(|e| err(&e))?;
        }
        Format::WebP => {
            let encoded = webp::Encoder::from_rgba(image.as_raw(), w, h)
                .encode(f32::from(options.quality.clamped(1, 100)));
            out = encoded.to_vec();
            if let Some(exif) = &options.exif {
                out = webp_with_exif(&out, exif, w, h).ok_or_else(|| err(&"malformed WebP"))?;
            }
        }
        Format::Tiff => {
            let mut writer = Cursor::new(&mut out);
            let mut encoder = tiff::encoder::TiffEncoder::new(&mut writer).map_err(|e| err(&e))?;
            let mut tiff_image = encoder
                .new_image::<tiff::encoder::colortype::RGBA8>(w, h)
                .map_err(|e| err(&e))?;
            if let Some(dpi) = options.dpi {
                tiff_image.resolution(
                    tiff::tags::ResolutionUnit::Inch,
                    tiff::encoder::Rational {
                        n: (dpi * 100.0).round() as u32,
                        d: 100,
                    },
                );
            }
            tiff_image.write_data(image.as_raw()).map_err(|e| err(&e))?;
        }
        Format::Bmp => {
            let rgb = flatten_onto_white(image)?;
            let mut writer = BufWriter::new(Cursor::new(&mut out));
            image::codecs::bmp::BmpEncoder::new(&mut writer)
                .write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map_err(|e| err(&e))?;
            drop(writer);
            if let Some(dpi) = options.dpi {
                // BITMAPINFOHEADER resolution fields, pixels per meter.
                let ppm = ((dpi / 0.0254).round() as i32).to_le_bytes();
                if let Some(fields) = out.get_mut(38..46) {
                    for (dst, src) in fields.iter_mut().zip(ppm.iter().chain(&ppm)) {
                        *dst = *src;
                    }
                }
            }
        }
        Format::Gif => return Err(err(&"GIF export is not supported")),
        Format::Heif | Format::Avif => {
            out = heif::encode(image, options)?;
        }
    }
    Ok(out)
}

/// Encodes and writes an image file.
pub fn write(image: &RgbaImage, path: &Path, options: &EncodeOptions) -> Result<(), CodecError> {
    let bytes = encode(image, options)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Inserts a `pHYs` chunk after `IHDR` (PNG has no other way to carry DPI).
fn png_with_dpi(png: &[u8], dpi: f64) -> Option<Vec<u8>> {
    // 8-byte signature, then IHDR: 4 length + 4 type + 13 data + 4 crc.
    const AFTER_IHDR: usize = 8 + 25;
    let ppm = (dpi / 0.0254).round() as u32;
    let mut chunk = Vec::with_capacity(21);
    chunk.extend_from_slice(&9u32.to_be_bytes());
    let mut body = b"pHYs".to_vec();
    body.extend_from_slice(&ppm.to_be_bytes());
    body.extend_from_slice(&ppm.to_be_bytes());
    body.push(1); // unit: meter
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    let (head, tail) = png.split_at_checked(AFTER_IHDR)?;
    let mut out = Vec::with_capacity(png.len().saturating_add(chunk.len()));
    out.extend_from_slice(head);
    out.extend_from_slice(&chunk);
    out.extend_from_slice(tail);
    Some(out)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Adds an `EXIF` chunk to a WebP file, converting it to the extended (`VP8X`) layout.
fn webp_with_exif(webp: &[u8], exif: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let (header, mut rest) = webp.split_at_checked(12)?;
    if header.get(..4)? != b"RIFF" || header.get(8..)? != b"WEBP" {
        return None;
    }
    let mut chunks: Vec<([u8; 4], &[u8])> = Vec::new();
    while let Some((chunk_header, after)) = rest.split_at_checked(8) {
        let (tag, len) = chunk_header.split_at_checked(4)?;
        let tag: [u8; 4] = *tag.first_chunk()?;
        let Ok(len) = usize::try_from(u32::from_le_bytes(*len.first_chunk()?)) else {
            return None;
        };
        let data = after.get(..len)?;
        chunks.push((tag, data));
        rest = after.get(len.checked_add(len & 1)?..).unwrap_or_default();
    }
    let has_alpha = chunks.iter().any(|(tag, _)| tag == b"ALPH");
    let mut vp8x: [u8; 10] = if let Some((_, data)) = chunks.iter().find(|(tag, _)| tag == b"VP8X")
    {
        *data.first_chunk()?
    } else {
        let [w0, w1, w2, _] = width.checked_sub(1)?.to_le_bytes();
        let [h0, h1, h2, _] = height.checked_sub(1)?.to_le_bytes();
        [0, 0, 0, 0, w0, w1, w2, h0, h1, h2]
    };
    vp8x[0] |= 0x08; // EXIF present
    if has_alpha {
        vp8x[0] |= 0x10;
    }
    let mut body = b"WEBP".to_vec();
    let mut push = |tag: &[u8], data: &[u8]| {
        body.extend_from_slice(tag);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() & 1 == 1 {
            body.push(0);
        }
    };
    push(b"VP8X", &vp8x);
    for (tag, data) in chunks
        .iter()
        .filter(|(tag, _)| tag != b"VP8X" && tag != b"EXIF")
    {
        push(tag, data);
    }
    push(b"EXIF", exif);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Some(out)
}

mod heif {
    use image::RgbaImage;
    use libheif_rs::{
        Channel, ColorSpace, CompressionFormat, EncoderQuality, HeifContext, Image, LibHeif,
        RgbChroma,
    };

    use super::{CodecError, Decoded, EncodeOptions, Format};
    use crate::panic_free::{self, Clamped};

    pub fn decode(bytes: &[u8]) -> Result<Decoded, CodecError> {
        let err = |e: libheif_rs::HeifError| CodecError::Decode(e.to_string());
        let lib = LibHeif::new();
        let context = HeifContext::read_from_bytes(bytes).map_err(err)?;
        let handle = context.primary_image_handle().map_err(err)?;
        let mut ids = vec![0; handle.number_of_metadata_blocks(b"Exif").max(0) as usize];
        let count = handle.metadata_block_ids(&mut ids, b"Exif");
        let exif = ids
            .iter()
            .take(count)
            .next()
            // unreadable EXIF is treated as none, so the image still opens
            .and_then(|&id| match handle.metadata(id) {
                Ok(block) => Some(block),
                Err(_) => None,
            })
            .and_then(|block| {
                // Exif blocks in HEIF start with a 4-byte offset to the TIFF header.
                let offset = u32::from_be_bytes(*block.first_chunk()?) as usize;
                block.get(offset.checked_add(4)?..).map(<[u8]>::to_vec)
            });
        let image = lib
            .decode(&handle, ColorSpace::Rgb(RgbChroma::Rgba), None)
            .map_err(err)?;
        let plane = image
            .planes()
            .interleaved
            .ok_or_else(|| CodecError::Decode("no interleaved plane".into()))?;
        let (w, h) = (plane.width, plane.height);
        let row_len = (w as usize).saturating_mul(4);
        let mut pixels = Vec::with_capacity(row_len.saturating_mul(h as usize));
        // a zero stride fails the row length check
        for row in panic_free::chunks(plane.data, plane.stride).take(h as usize) {
            let row = row
                .get(..row_len)
                .ok_or_else(|| CodecError::Decode("unexpected plane size".into()))?;
            pixels.extend_from_slice(row);
        }
        let pixels = RgbaImage::from_raw(w, h, pixels)
            .ok_or_else(|| CodecError::Decode("unexpected plane size".into()))?;
        Ok(Decoded { pixels, exif })
    }

    pub fn encode(image: &RgbaImage, options: &EncodeOptions) -> Result<Vec<u8>, CodecError> {
        let err = |e: libheif_rs::HeifError| CodecError::Encode(e.to_string());
        let (w, h) = image.dimensions();
        let lib = LibHeif::new();
        let compression = if options.format == Format::Avif {
            CompressionFormat::Av1
        } else {
            CompressionFormat::Hevc
        };
        let mut encoder = lib.encoder_for_format(compression).map_err(|e| {
            CodecError::Encode(format!(
                "no {} encoder is installed ({e})",
                options.format.label()
            ))
        })?;
        encoder
            .set_quality(EncoderQuality::Lossy(options.quality.clamped(1, 100)))
            .map_err(err)?;
        let mut heif_image = Image::new(w, h, ColorSpace::Rgb(RgbChroma::Rgba)).map_err(err)?;
        heif_image
            .create_plane(Channel::Interleaved, w, h, 8)
            .map_err(err)?;
        {
            let planes = heif_image.planes_mut();
            let plane = planes
                .interleaved
                .ok_or_else(|| CodecError::Encode("no interleaved plane".into()))?;
            let row_len = (w as usize).saturating_mul(4);
            // a zero stride fails the row length check
            for (dst, src) in panic_free::chunks_mut(plane.data, plane.stride)
                .zip(panic_free::chunks(image.as_raw(), row_len))
            {
                if !panic_free::copy_into(dst, src) {
                    return Err(CodecError::Encode("unexpected plane size".into()));
                }
            }
        }
        let mut context = HeifContext::new().map_err(err)?;
        let handle = context
            .encode_image(&heif_image, &mut encoder, None)
            .map_err(err)?;
        if let Some(exif) = &options.exif {
            context.add_exif_metadata(&handle, exif).map_err(err)?;
        }
        context.write_to_bytes().map_err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_reference() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn flattening_uses_white() {
        let image = RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        assert_eq!(
            flatten_onto_white(&image).unwrap().get_pixel(0, 0).0,
            [255, 255, 255]
        );
    }
}
