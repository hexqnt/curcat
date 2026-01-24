use anyhow::{Context as _, bail};
use bincode::config::{Config, standard};
use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};
use std::fs;
use std::path::Path;

use super::model::{ProjectLoadOutcome, ProjectPayload, ProjectPayloadV1};
use super::path::{resolve_image_path, write_atomic};

/// Magic signature prefix for project files
pub const PROJECT_MAGIC: &[u8; 6] = b"CURCAT";
/// Current binary project format version.
pub const PROJECT_VERSION: u32 = 2;

fn bincode_config() -> impl Config {
    standard().with_little_endian()
}

fn encode_payload(payload: &ProjectPayload) -> anyhow::Result<Vec<u8>> {
    bincode::serde::encode_to_vec(payload, bincode_config())
        .context("Failed to serialize project payload")
}

fn decode_payload(bytes: &[u8]) -> anyhow::Result<ProjectPayload> {
    let (payload, _): (ProjectPayload, usize) =
        bincode::serde::decode_from_slice(bytes, bincode_config())
            .context("Failed to deserialize project payload")?;
    Ok(payload)
}

fn decode_payload_v1(bytes: &[u8]) -> anyhow::Result<ProjectPayloadV1> {
    let (payload, _): (ProjectPayloadV1, usize) =
        bincode::serde::decode_from_slice(bytes, bincode_config())
            .context("Failed to deserialize v1 project payload")?;
    Ok(payload)
}

/// Save a project with compression and an atomic temp-file swap.
pub fn save_project(path: &Path, payload: &ProjectPayload) -> anyhow::Result<()> {
    let encoded = encode_payload(payload)?;
    let compressed = compress_prepend_size(&encoded);
    let mut buffer = Vec::with_capacity(PROJECT_MAGIC.len() + 4 + compressed.len());
    buffer.extend_from_slice(PROJECT_MAGIC);
    buffer.extend_from_slice(&PROJECT_VERSION.to_le_bytes());
    buffer.extend_from_slice(&compressed);
    write_atomic(path, &buffer)
}

/// Load a project: validate header, decompress, and resolve the image path.
pub fn load_project(path: &Path) -> anyhow::Result<ProjectLoadOutcome> {
    let bytes =
        fs::read(path).with_context(|| format!("Failed to read project {}", path.display()))?;
    let header_len = PROJECT_MAGIC.len() + std::mem::size_of::<u32>();
    if bytes.len() < header_len {
        bail!("Project file too small or missing header");
    }
    let (magic, rest) = bytes.split_at(PROJECT_MAGIC.len());
    if magic != PROJECT_MAGIC {
        bail!("Not a Curcat project file: magic signature mismatch");
    }
    let (version_bytes, compressed) = rest.split_at(std::mem::size_of::<u32>());
    let version = u32::from_le_bytes(version_bytes.try_into().unwrap_or_default());
    let decompressed =
        decompress_size_prepended(compressed).context("Failed to decompress project payload")?;
    let payload = match version {
        1 => ProjectPayload::from(decode_payload_v1(&decompressed)?),
        PROJECT_VERSION => decode_payload(&decompressed)?,
        _ => {
            bail!("Unsupported project version {version}. Supported versions: 1, {PROJECT_VERSION}")
        }
    };
    let (chosen_image, warnings) = resolve_image_path(path, &payload)?;
    Ok(ProjectLoadOutcome {
        payload,
        chosen_image,
        warnings,
        version,
    })
}
