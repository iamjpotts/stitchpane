//! EXIF metadata: grouping for the viewer (META-003) and rewriting for export (META-005).

use std::io::Cursor;

use exif::{Field, In, Tag, Value};

/// Groups shown in the viewer and offered as export checkboxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExifGroup {
    Camera,
    CaptureSettings,
    DateTime,
    Location,
    Other,
}

impl ExifGroup {
    pub const ALL: [ExifGroup; 5] = [
        ExifGroup::Camera,
        ExifGroup::CaptureSettings,
        ExifGroup::DateTime,
        ExifGroup::Location,
        ExifGroup::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExifGroup::Camera => "Camera",
            ExifGroup::CaptureSettings => "Capture settings",
            ExifGroup::DateTime => "Date and time",
            ExifGroup::Location => "Location",
            ExifGroup::Other => "Other",
        }
    }

    /// Whether the export checkbox starts checked (META-005: all but location).
    pub fn exported_by_default(self) -> bool {
        self != ExifGroup::Location
    }

    fn of(tag: Tag) -> ExifGroup {
        const CAMERA: &[Tag] = &[
            Tag::Make,
            Tag::Model,
            Tag::LensMake,
            Tag::LensModel,
            Tag::LensSpecification,
            Tag::LensSerialNumber,
            Tag::BodySerialNumber,
            Tag::CameraOwnerName,
        ];
        const CAPTURE: &[Tag] = &[
            Tag::ExposureTime,
            Tag::FNumber,
            Tag::ExposureProgram,
            Tag::PhotographicSensitivity,
            Tag::SensitivityType,
            Tag::ExposureBiasValue,
            Tag::MaxApertureValue,
            Tag::ApertureValue,
            Tag::ShutterSpeedValue,
            Tag::BrightnessValue,
            Tag::MeteringMode,
            Tag::LightSource,
            Tag::Flash,
            Tag::FocalLength,
            Tag::FocalLengthIn35mmFilm,
            Tag::WhiteBalance,
            Tag::ExposureMode,
            Tag::DigitalZoomRatio,
            Tag::SceneCaptureType,
            Tag::Contrast,
            Tag::Saturation,
            Tag::Sharpness,
            Tag::SubjectDistance,
            Tag::SubjectDistanceRange,
            Tag::GainControl,
            Tag::SceneType,
            Tag::SensingMethod,
        ];
        const DATE_TIME: &[Tag] = &[
            Tag::DateTime,
            Tag::DateTimeOriginal,
            Tag::DateTimeDigitized,
            Tag::OffsetTime,
            Tag::OffsetTimeOriginal,
            Tag::OffsetTimeDigitized,
            Tag::SubSecTime,
            Tag::SubSecTimeOriginal,
            Tag::SubSecTimeDigitized,
        ];
        if tag.context() == exif::Context::Gps {
            ExifGroup::Location
        } else if CAMERA.contains(&tag) {
            ExifGroup::Camera
        } else if CAPTURE.contains(&tag) {
            ExifGroup::CaptureSettings
        } else if DATE_TIME.contains(&tag) {
            ExifGroup::DateTime
        } else {
            ExifGroup::Other
        }
    }
}

/// One row of the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExifEntry {
    pub group: ExifGroup,
    pub name: String,
    pub value: String,
}

/// Parsed EXIF metadata of one source image (primary image only; thumbnails dropped).
#[derive(Clone, Debug)]
pub struct Exif {
    fields: Vec<Field>,
    entries: Vec<ExifEntry>,
    little_endian: bool,
}

fn is_structural(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::ExifIFDPointer
            | Tag::GPSInfoIFDPointer
            | Tag::InteropIFDPointer
            | Tag::StripOffsets
            | Tag::StripByteCounts
            | Tag::TileOffsets
            | Tag::TileByteCounts
            | Tag::JPEGInterchangeFormat
            | Tag::JPEGInterchangeFormatLength
    )
}

impl Exif {
    /// Parses TIFF-structured EXIF data. Returns `None` if it is unreadable or empty.
    pub fn parse(raw: &[u8]) -> Option<Exif> {
        let raw = raw.strip_prefix(b"Exif\0\0").unwrap_or(raw);
        let exif = exif::Reader::new().read_raw(raw.to_vec()).ok()?;
        let fields: Vec<Field> = exif
            .fields()
            .filter(|f| f.ifd_num == In::PRIMARY && !is_structural(f.tag))
            .cloned()
            .collect();
        if fields.is_empty() {
            return None;
        }
        let mut entries: Vec<ExifEntry> = fields
            .iter()
            .map(|f| ExifEntry {
                group: ExifGroup::of(f.tag),
                name: f
                    .tag
                    .description()
                    .map(str::to_owned)
                    .unwrap_or_else(|| f.tag.to_string()),
                value: f.display_value().with_unit(&exif).to_string(),
            })
            .collect();
        entries.sort_by_key(|e| e.group);
        Some(Exif {
            fields,
            entries,
            little_endian: exif.little_endian(),
        })
    }

    /// Rows for the viewer, grouped (META-003).
    pub fn entries(&self) -> &[ExifEntry] {
        &self.entries
    }

    /// Groups that have at least one field.
    pub fn groups(&self) -> Vec<ExifGroup> {
        let mut groups: Vec<ExifGroup> = self.fields.iter().map(|f| ExifGroup::of(f.tag)).collect();
        groups.sort();
        groups.dedup();
        groups
    }

    /// Encodes the chosen groups for an exported image of the given size, with
    /// orientation reset to upright, pixel dimensions updated, and the resolution taken
    /// from the canvas DPI rather than the source (META-005, EXPORT-002).
    pub fn export(
        &self,
        groups: &[ExifGroup],
        width: u32,
        height: u32,
        dpi: Option<f64>,
    ) -> Option<Vec<u8>> {
        // Tags that describe the source file's own encoding or are rewritten below.
        let replaced = [
            Tag::Orientation,
            Tag::PixelXDimension,
            Tag::PixelYDimension,
            Tag::XResolution,
            Tag::YResolution,
            Tag::ResolutionUnit,
            Tag::ImageWidth,
            Tag::ImageLength,
            Tag::Compression,
            Tag::BitsPerSample,
            Tag::PhotometricInterpretation,
            Tag::SamplesPerPixel,
            Tag::PlanarConfiguration,
            Tag::RowsPerStrip,
            Tag::YCbCrSubSampling,
        ];
        let mut fields: Vec<Field> = self
            .fields
            .iter()
            .filter(|f| groups.contains(&ExifGroup::of(f.tag)) && !replaced.contains(&f.tag))
            // The writer cannot encode values of unknown type.
            .filter(|f| !matches!(f.value, Value::Unknown(..)))
            .cloned()
            .collect();
        if fields.is_empty() {
            return None;
        }
        let field = |tag, value| Field {
            tag,
            ifd_num: In::PRIMARY,
            value,
        };
        fields.push(field(Tag::Orientation, Value::Short(vec![1])));
        fields.push(field(Tag::PixelXDimension, Value::Long(vec![width])));
        fields.push(field(Tag::PixelYDimension, Value::Long(vec![height])));
        if let Some(dpi) = dpi {
            let r = exif::Rational {
                num: (dpi * 100.0).round() as u32,
                denom: 100,
            };
            fields.push(field(Tag::XResolution, Value::Rational(vec![r])));
            fields.push(field(Tag::YResolution, Value::Rational(vec![r])));
            fields.push(field(Tag::ResolutionUnit, Value::Short(vec![2])));
        }
        let mut writer = exif::experimental::Writer::new();
        for field in &fields {
            writer.push_field(field);
        }
        let mut out = Cursor::new(Vec::new());
        writer.write(&mut out, self.little_endian).ok()?;
        Some(out.into_inner())
    }
}
