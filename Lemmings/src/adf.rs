// adf.rs — Amiga ADF disk reader for Lemmings
// The Lemmings disk uses a custom DMA Design bootblock loader.
// There is NO ASCII index table — file offsets are embedded in M68000 code.
// Strategy: scan both ADF images for RNC1 magic bytes to locate every
// compressed file, read its size from the RNC header, then map files to
// their canonical names by their order of appearance on each disk.
// Language: Rust, file: src/adf.rs

use anyhow::{bail, Context, Result};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

pub const ADF_SIZE: usize = 901_120; // 80 tracks × 2 sides × 11 sectors × 512 bytes

const RNC1_MAGIC: &[u8; 4] = b"RNC1";
const RNC2_MAGIC: &[u8; 4] = b"RNC2";

// ── Known file order on Lemmings Disk 1 ──────────────────────────────────────
// Derived from community reverse engineering of the DMA Design custom loader.
// Files appear in this order, packed sequentially after the bootblock.
const DISK1_NAMES: &[&str] = &[
    "main", "Code", "noises",
    "Ground1", "Ground2", "Ground3", "Ground4", "Ground5",
    "Objects1", "Objects2", "Objects3", "Objects4", "Objects5",
    "Icons",
    "Level000","Level001","Level002","Level003","Level004",
    "Level005","Level006","Level007","Level008","Level009",
    "Level010","Level011","Level012","Level013","Level014",
    "Level015","Level016","Level017","Level018","Level019",
    "Level020","Level021","Level022","Level023","Level024",
];

// ── Known file order on Lemmings Disk 2 ──────────────────────────────────────
const DISK2_NAMES: &[&str] = &[
    "tim1","tim2","tim3","tim4","tim5",
    "tim6","tim7","tim8","tim9","tim10",
];

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name:   String,
    pub offset: usize,  // byte offset into the combined data vec
    pub size:   usize,  // compressed byte length (as stored)
}

pub struct Disk {
    data:    Vec<u8>,
    entries: Vec<FileEntry>,
}

impl Disk {
    pub fn load(disk1: &[u8], disk2: &[u8]) -> Result<Self> {
        if disk1.len() != ADF_SIZE {
            bail!("Disk 1 wrong size: {} bytes (expected {})", disk1.len(), ADF_SIZE);
        }
        if disk2.len() != ADF_SIZE {
            bail!("Disk 2 wrong size: {} bytes (expected {})", disk2.len(), ADF_SIZE);
        }

        // Concatenate both disks; disk2 starts at ADF_SIZE
        let mut data = Vec::with_capacity(ADF_SIZE * 2);
        data.extend_from_slice(disk1);
        data.extend_from_slice(disk2);

        // Scan disk1 region for RNC files
        let mut entries = Vec::new();
        scan_rnc(&data[..ADF_SIZE], 0, DISK1_NAMES, &mut entries);

        // Scan disk2 region for RNC files (offsets shifted by ADF_SIZE)
        scan_rnc(&data[ADF_SIZE..], ADF_SIZE, DISK2_NAMES, &mut entries);

        log::info!("ADF scan found {} files", entries.len());
        for e in &entries {
            log::debug!("  {:12}  offset=0x{:06X}  size={}", e.name, e.offset, e.size);
        }

        if entries.is_empty() {
            bail!(
                "No RNC-compressed files found in ADF images.\n\
                 Ensure you are using the original Amiga Lemmings ADF files.\n\
                 The files must be named Lemmings.1of2.adf and Lemmings.2of2.adf."
            );
        }

        Ok(Self { data, entries })
    }

    pub fn file_count(&self) -> usize { self.entries.len() }
    pub fn entries(&self)    -> &[FileEntry] { &self.entries }

    /// Return raw (still-compressed) bytes for a named file.
    pub fn raw_file(&self, name: &str) -> Option<&[u8]> {
        let e = self.entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))?;
        self.data.get(e.offset..e.offset + e.size)
    }

    /// Decompress and return a file's bytes.
    pub fn file(&self, name: &str) -> Result<Vec<u8>> {
        let raw = self.raw_file(name)
            .with_context(|| format!("File '{}' not found in ADF", name))?;
        crate::decompress::decompress(raw)
            .with_context(|| format!("Decompression failed for '{}'", name))
    }

    /// Return raw bytes without decompression (for already-raw assets).
    pub fn file_raw(&self, name: &str) -> Result<Vec<u8>> {
        Ok(self.raw_file(name)
            .with_context(|| format!("File '{}' not found in ADF (raw)", name))?
            .to_vec())
    }
}

// ── RNC scanner ──────────────────────────────────────────────────────────────

/// Walk `region` looking for RNC1/RNC2 magic bytes.
/// Each found file is assigned the next name from `names`.
/// `base_offset` is added to every found offset so it refers into the full
/// combined data vec rather than just this region.
fn scan_rnc(region: &[u8], base_offset: usize, names: &[&str], out: &mut Vec<FileEntry>) {
    let mut pos   = 0;
    let mut name_idx = 0;

    while pos + 18 <= region.len() {
        let magic = &region[pos..pos + 4];
        if magic == RNC1_MAGIC || magic == RNC2_MAGIC {
            // RNC header: [magic:4][unpacked_len:4][packed_len:4][...]
            if pos + 12 <= region.len() {
                let packed_len = u32::from_be_bytes([
                    region[pos + 8],
                    region[pos + 9],
                    region[pos + 10],
                    region[pos + 11],
                ]) as usize;

                let total_size = 18 + packed_len; // 18-byte header + data

                if pos + total_size <= region.len() && packed_len > 0 && packed_len < ADF_SIZE {
                    let name = if name_idx < names.len() {
                        names[name_idx].to_string()
                    } else {
                        format!("file_{:03}", name_idx)
                    };
                    name_idx += 1;

                    out.push(FileEntry {
                        name,
                        offset: base_offset + pos,
                        size:   total_size,
                    });

                    // Jump past this file to avoid re-triggering on internal data
                    pos += total_size;
                    continue;
                }
            }
        }
        pos += 1;
    }
}
