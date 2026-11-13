use super::{mutation_allowed, AppState};
use tauri::State;

/// Maximum size accepted for a pasted image (20 MB).
const MAX_IMAGE_SIZE: usize = 20 * 1024 * 1024;
/// PNG header (`\x89PNG\r\n\x1a\n`).
const MAGIC_PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Validates that a blob really is a PNG or JPEG image and returns its extension.
///
/// Without this check, arbitrary bytes from the webview were written as is to `.png`
/// (misleading file, fillable disk). Pure function: testable without I/O.
fn validate_image(bytes: &[u8]) -> Result<&'static str, String> {
    if bytes.len() > MAX_IMAGE_SIZE {
        return Err(crate::tr!("data.imageTooLarge", size = bytes.len()));
    }
    if bytes.starts_with(&MAGIC_PNG) {
        Ok("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Ok("jpg")
    } else {
        Err(crate::tr!("backup.imageFormatUnknown"))
    }
}

/// Folder of the images pasted into notes.
fn images_dir() -> Result<std::path::PathBuf, String> {
    Ok(crate::data_location::data_dir()?.join(crate::data_location::IMAGES_DIR))
}

/// Path of the images folder: the interface needs it to display a portable reference
/// `humcase-image://<name>` (see `crate::images`).
#[tauri::command]
pub fn get_images_dir() -> Result<String, String> {
    Ok(images_dir()?.to_string_lossy().to_string())
}

/// Saves a pasted image, compressed (see `crate::images::prepare`), and returns its
/// portable reference `humcase-image://<name>` to insert in the note.
#[tauri::command]
pub fn save_image_to_disk(state: State<'_, AppState>, bytes: Vec<u8>) -> Result<String, String> {
    let _mutation = mutation_allowed(&state)?;
    let extension = validate_image(&bytes)?;
    let prepare = crate::images::prepare(&bytes, extension);
    // Atomic write then registration in the `images` registry (v19): see `crate::images::store_image`.
    crate::images::store_image(&state.db, &images_dir()?, &prepare)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P2-R2 — a non-image blob (or one that is too large) is refused; PNG and JPEG are accepted.
    #[test]
    fn test_validate_image() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert_eq!(validate_image(&png).unwrap(), "png");
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0];
        assert_eq!(validate_image(&jpg).unwrap(), "jpg");
        assert!(validate_image(b"not an image").is_err());
        assert!(validate_image(&[]).is_err());

        let mut too_big = vec![0u8; MAX_IMAGE_SIZE + 1];
        too_big[..8].copy_from_slice(&MAGIC_PNG);
        assert!(
            validate_image(&too_big).is_err(),
            "beyond 20 MB must be refused"
        );
    }
}
