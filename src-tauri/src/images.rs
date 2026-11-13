//! Note images: portable reference and compression at save time.
//!
//! Before 0.9.81, a note referenced its images by a URL `http://asset.localhost/<encoded absolute
//! path>`: tied to this PC and this Windows account, it breaks on another machine and on the web.
//! The content now keeps a portable reference `humcase-image://<name>`; only display
//! translates it into a local URL. Old URLs remain displayable and are converted by the v16
//! migration and on every save from the editor.

use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use percent_encoding::percent_decode_str;
use regex::Regex;
use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Cursor;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

/// Prefix of the portable reference stored in the notes' Markdown.
pub const SCHEME: &str = "humcase-image://";

/// Longest side kept: beyond it, the image is downscaled (text of a 4K screenshot still readable).
const MAX_SIDE: u32 = 3840;
/// JPEG quality after downscaling a photo.
const JPEG_QUALITY: u8 = 85;

pub fn reference(name: &str) -> String {
    format!("{SCHEME}{name}")
}

/// Safe image file name: no path, simple characters (same rule as archives).
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn asset_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"(?i)(?:https?://asset\.localhost|asset://localhost)/[^\s<>\[\)"']+"#).unwrap()
    })
}

/// Name of the image if the local URL points into Humcase's `images` folder (old or new
/// data folder), `None` for any other URL (a file elsewhere on the disk, for example).
fn internal_image_name(url: &str) -> Option<String> {
    let path = url.split_once("://")?.1.split_once('/')?.1;
    let decoded = percent_decode_str(path)
        .decode_utf8_lossy()
        .replace('\\', "/");
    let start = images_segment_end(&decoded.to_ascii_lowercase())?;
    let name = &decoded[start..];
    is_valid_name(name).then(|| name.to_string())
}

/// Position just after `/<data folder>/images/` in a lowercase `/` path, for Humcase's data
/// folder `.humcase`, the AppData folder `com.humcase.desktop` of versions before 0.10.0 or the
/// `com.<owner>.devnotes` folder of versions up to 0.9.72 (older contents and archives still
/// point there). `None` for any other path.
pub(crate) fn images_segment_end(lower: &str) -> Option<usize> {
    const IMAGES: &str = "/images/";
    lower.match_indices(IMAGES).find_map(|(index, _)| {
        let folder = lower[..index].rsplit('/').next()?;
        let former = folder.starts_with("com.") && folder.ends_with(".devnotes");
        (folder == crate::data_location::DIR_NAME
            || folder == crate::data_migration::APP_ID
            || former)
            .then_some(index + IMAGES.len())
    })
}

/// Replaces local URLs of internal images with their portable reference; the rest is intact.
pub fn make_portable(content: &str) -> String {
    asset_regex()
        .replace_all(content, |caps: &regex::Captures<'_>| {
            let url = caps.get(0).unwrap().as_str();
            internal_image_name(url)
                .map(|name| reference(&name))
                .unwrap_or_else(|| url.to_string())
        })
        .into_owned()
}

/// Image ready to write: bytes and extension.
pub struct PreparedImage {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Compresses an already validated PNG or JPEG image.
///
/// - PNG (screenshots): lossless WebP (libwebp), kept only if it is lighter.
/// - JPEG (photos): orientation applied and metadata removed (GPS position included);
///   downscaling beyond [`MAX_SIDE`] and re-encoding, kept only if it is lighter.
///
/// Any decoding error keeps the original as is: compression never makes a pasted
/// image get lost.
pub fn prepare(bytes: &[u8], extension: &'static str) -> PreparedImage {
    let original = || PreparedImage {
        bytes: bytes.to_vec(),
        extension,
    };
    let format = match extension {
        "png" => ImageFormat::Png,
        "jpg" => ImageFormat::Jpeg,
        _ => return original(),
    };
    let Ok(image) = decode(bytes, format) else {
        return original();
    };
    let image = downscale(image);
    let compressed = match format {
        ImageFormat::Png => encode(&image, ImageFormat::WebP).map(|o| (o, "webp")),
        _ => encode(&image, ImageFormat::Jpeg).map(|o| (o, "jpg")),
    };
    match compressed {
        Some((output, ext)) if output.len() < bytes.len() => PreparedImage {
            bytes: output,
            extension: ext,
        },
        _ => original(),
    }
}

fn decode(bytes: &[u8], format: ImageFormat) -> Result<DynamicImage, image::ImageError> {
    let mut decoder = ImageReader::with_format(Cursor::new(bytes), format).into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn downscale(image: DynamicImage) -> DynamicImage {
    if image.width().max(image.height()) <= MAX_SIDE {
        return image;
    }
    image.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Lanczos3)
}

fn encode(image: &DynamicImage, format: ImageFormat) -> Option<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    match format {
        ImageFormat::Jpeg => {
            let rgb = image.to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, JPEG_QUALITY)
                .encode_image(&rgb)
                .ok()?;
        }
        // Lossless libwebp: -52% on 90 real screenshots (10.7 → 5.1 MB, ~50 ms each),
        // identical pixels. The pure Rust WebP encoder of `image` gained almost nothing.
        _ => {
            let rgba = image.to_rgba8();
            let (width, height) = rgba.dimensions();
            return Some(
                webp::Encoder::from_rgba(&rgba, width, height)
                    .encode_lossless()
                    .to_vec(),
            );
        }
    }
    Some(output.into_inner())
}

/// Lowercase hexadecimal of a fingerprint, the form stored in the `images` registry and in
/// archive manifests.
pub fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Size and SHA-256 fingerprint (lowercase hexadecimal) of a file, read as a stream.
pub fn hash_file(path: &Path) -> std::io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let count = std::io::Read::read(&mut file, &mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((size, lower_hex(&hasher.finalize())))
}

fn hash_bytes(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

/// Records (or updates) an image in the `images` registry (v19): the registry describes the
/// file as it is on disk, the provided fingerprint is authoritative. `created_at` of an
/// existing row is kept. Works inside a transaction opened by the caller (the `&Connection`
/// of a `Transaction` dereferences), which archive restore uses.
pub fn register_image(
    conn: &Connection,
    name: &str,
    sha256: &str,
    size: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO images (name, sha256, size, created_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
         ON CONFLICT(name) DO UPDATE SET sha256 = excluded.sha256, size = excluded.size",
        params![name, sha256, size as i64],
    )?;
    Ok(())
}

/// Fills the registry from the existing folder: one row per image file not yet known.
///
/// Idempotent and cheap when the registry is up to date (one listing and one read of the table:
/// an already registered file is not re-read, image names are never reused for other
/// content). **Never deletes, moves or modifies a file**: what cannot be read
/// (permissions, symbolic link, folder, non-conforming name) is reported on stderr — never stdout, reserved
/// for the MCP — and ignored. `.tmp` files (atomic write of an image in progress) are silently ignored.
/// A registry row whose file has disappeared is left as is. Returns the number of
/// rows added.
pub fn sync_registry(conn: &Connection, images_dir: &Path) -> Result<usize, String> {
    if !images_dir.exists() {
        return Ok(0);
    }
    let mut known: HashSet<String> = conn
        .prepare("SELECT name FROM images")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    for item in fs::read_dir(images_dir).map_err(|e| format!("{}: {e}", images_dir.display()))? {
        let item = match item {
            Ok(item) => item,
            Err(e) => {
                eprintln!("humcase: image skipped in {}: {e}", images_dir.display());
                continue;
            }
        };
        let Ok(name) = item.file_name().into_string() else {
            eprintln!(
                "humcase: image skipped (non-UTF-8 name) in {}: {:?}",
                images_dir.display(),
                item.file_name()
            );
            continue;
        };
        if name.ends_with(".tmp") || known.contains(&name) {
            continue;
        }
        if !is_valid_name(&name) {
            eprintln!("humcase: image skipped (non-conforming name): {name}");
            continue;
        }
        let path = item.path();
        // `symlink_metadata` does not follow links: as for archives, a link is not an image.
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => {
                eprintln!("humcase: image skipped (not a regular file): {name}");
                continue;
            }
            Err(e) => {
                eprintln!("humcase: image skipped ({name}): {e}");
                continue;
            }
        };
        let (size, sha256) = match hash_file(&path) {
            Ok(hashed) => hashed,
            Err(e) => {
                eprintln!("humcase: unreadable image, skipped ({name}): {e}");
                continue;
            }
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_secs_f64());
        known.insert(name.clone());
        found.push((name, sha256, size, modified));
    }
    if found.is_empty() {
        return Ok(0);
    }
    // A single transaction, taken after hashing: the write lock is not held while
    // reading files. `OR IGNORE`: another process (MCP, CLI) may have registered the same row.
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let mut added = 0;
    for (name, sha256, size, modified) in found {
        added += tx
            .execute(
                "INSERT OR IGNORE INTO images (name, sha256, size, created_at)
                 VALUES (?1, ?2, ?3, COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', ?4, 'unixepoch'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')))",
                params![name, sha256, size as i64, modified],
            )
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(added)
}

/// Writes a prepared image into `images_dir` under a new name, atomically (temporary file
/// then rename), and records it in the registry. Returns the portable reference.
///
/// The file is written **before** registration: a registry failure never loses the image (the
/// reference is returned, the missing row is caught up by `sync_registry` at the next
/// startup) and a write failure registers nothing.
pub fn store_image(
    db: &Mutex<Connection>,
    images_dir: &Path,
    prepared: &PreparedImage,
) -> Result<String, String> {
    fs::create_dir_all(images_dir).map_err(|e| e.to_string())?;
    let filename = format!("{}.{}", uuid::Uuid::new_v4(), prepared.extension);
    let filepath = images_dir.join(&filename);
    // Atomic write (temporary + rename), consistent with the AI settings: a power cut does not
    // leave a partial file.
    let temporary = images_dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temporary, &prepared.bytes).map_err(|e| e.to_string())?;
    fs::rename(&temporary, &filepath).map_err(|e| e.to_string())?;

    let registered = db.lock().map_err(|e| e.to_string()).and_then(|conn| {
        register_image(
            &conn,
            &filename,
            &hash_bytes(&prepared.bytes),
            prepared.bytes.len() as u64,
        )
        .map_err(|e| e.to_string())
    });
    if let Err(e) = registered {
        eprintln!("humcase: image {filename} saved but not registered in the registry: {e}");
    }
    Ok(reference(&filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_keep_the_form_of_existing_archives_and_registry() {
        // SHA-256 of "abc": archives and the `images` registry store this exact lowercase form.
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(hash_bytes(b"abc"), expected);
        let path = std::env::temp_dir().join(format!("humcase-hash-{}.bin", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        let hashed = hash_file(&path);
        let _ = fs::remove_file(&path);
        assert_eq!(hashed.unwrap(), (3, expected.to_string()));
    }

    #[test]
    fn internal_asset_urls_become_portable_and_others_are_kept() {
        let internal = "![a](http://asset.localhost/C%3A%5CUsers%5Cx%5CAppData%5CRoaming%5Ccom.humcase.desktop%5Cimages%5Cabc-1.png)";
        let legacy =
            "![b](asset://localhost/C:/Users/x/AppData/Roaming/com.example.devnotes/images/def.jpg)";
        let external = "![c](http://asset.localhost/C%3A%5CPhotos%5Cvacations.png) [d](https://example.com/x.png)";
        let result = make_portable(&format!("{internal}\n{legacy}\n{external}"));
        assert!(result.contains("![a](humcase-image://abc-1.png)"));
        assert!(result.contains("![b](humcase-image://def.jpg)"));
        assert!(
            result.contains(external),
            "an image outside the Humcase folder is not touched"
        );
        assert_eq!(make_portable(&result), result, "idempotent");
    }

    #[test]
    fn unsafe_names_are_not_rewritten() {
        let url = "![x](http://asset.localhost/C%3A%5Ccom.humcase.desktop%5Cimages%5C..%5C..%5Csecret.png)";
        assert_eq!(make_portable(url), url);
    }

    fn png_screenshot(width: u32, height: u32) -> Vec<u8> {
        // Typical screenshot: flat areas and text, highly compressible.
        let image = image::RgbaImage::from_fn(width, height, |x, y| {
            if (y / 20) % 2 == 0 && x % 7 < 5 {
                image::Rgba([30, 30, 30, 255])
            } else {
                image::Rgba([250, 250, 250, 255])
            }
        });
        let mut output = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut output, ImageFormat::Png)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn screenshots_become_lighter_webp_without_losing_pixels() {
        let png = png_screenshot(800, 600);
        let prepared = prepare(&png, "png");
        assert_eq!(prepared.extension, "webp");
        assert!(prepared.bytes.len() < png.len());
        let reread = image::load_from_memory(&prepared.bytes).unwrap();
        let original = image::load_from_memory(&png).unwrap();
        assert_eq!(
            reread.to_rgba8(),
            original.to_rgba8(),
            "lossless WebP: identical pixels"
        );
    }

    #[test]
    fn huge_images_are_reduced_and_undecodable_ones_kept_as_is() {
        // Noisy image (close to a photo): downscaling saves weight.
        let noise = image::RgbImage::from_fn(5000, 300, |x, y| {
            let v = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) as u8;
            image::Rgb([v, v.wrapping_mul(3), v.wrapping_add(7)])
        });
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(noise)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let prepared = prepare(&png, "png");
        let reread = image::load_from_memory(&prepared.bytes).unwrap();
        assert_eq!(reread.width(), MAX_SIDE);

        let fake = [0x89, b'P', b'N', b'G', 0, 1, 2, 3];
        let kept = prepare(&fake, "png");
        assert_eq!(kept.bytes, fake);
        assert_eq!(kept.extension, "png");
    }
}
