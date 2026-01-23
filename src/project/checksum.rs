use anyhow::Context as _;
use crc32fast::Hasher;
use std::fs;
use std::io::Read;
use std::path::Path;

/// Compute CRC32 of an image file.
pub fn compute_image_crc32(path: &Path) -> anyhow::Result<u32> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open image file for checksum: {}", path.display()))?;
    let mut hasher = Hasher::new();
    let mut buf = vec![0u8; 32 * 1024].into_boxed_slice();
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize())
}
