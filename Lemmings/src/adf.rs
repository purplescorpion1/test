// adf.rs — Amiga ADF disk reader for Lemmings
// The Lemmings disk uses an AmigaDOS custom file table layout at offset 0x0400.
// Directory entries are 16 bytes:
//   [0..10]:  File name (ASCII, null-padded)
//   [10..12]: Flags / reserved (0xFFFF for valid files)
//   [12..16]: Big-endian offset into the ADF disk image
//
// Language: Rust, file: src/adf.rs

use anyhow::{bail, Context, Result};

pub const ADF_SIZE: usize = 901_120; // 80 tracks × 2 sides × 11 sectors × 512 bytes
pub const DIR_START: usize = 0x0400;  // Sector 2 offset

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name:   String,
    pub offset: usize,  // byte offset into the combined data vec
    pub size:   usize,  // byte length as stored
    pub flag:   u16,
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

        let mut entries = Vec::new();

        // Scan disk 1 directory table at 0x0400
        scan_directory(disk1, 0, &mut entries);

        // Scan disk 2 directory table at 0x0400 (base offset ADF_SIZE)
        scan_directory(disk2, ADF_SIZE, &mut entries);

        log::info!("ADF scan found {} files", entries.len());
        for e in &entries {
            log::debug!("  {:12}  offset=0x{:06X}  size={}", e.name, e.offset, e.size);
        }

        if entries.is_empty() {
            bail!(
                "No directory entries found in ADF images.\n\
                 Ensure you are using the original Amiga Lemmings ADF files.\n\
                 The files must be named Lemmings.1of2.adf and Lemmings.2of2.adf."
            );
        }

        Ok(Self { data, entries })
    }

    pub fn file_count(&self) -> usize { self.entries.len() }
    pub fn entries(&self)    -> &[FileEntry] { &self.entries }

    /// Return raw bytes for a named file.
    pub fn raw_file(&self, name: &str) -> Option<&[u8]> {
        let e = self.entries.iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))?;
        self.data.get(e.offset..e.offset + e.size)
    }

    /// Return raw bytes for a file.
    pub fn file(&self, name: &str) -> Result<Vec<u8>> {
        Ok(self.file_raw(name)?)
    }

    /// Return raw bytes without decompression (assets are stored directly).
    pub fn file_raw(&self, name: &str) -> Result<Vec<u8>> {
        self.raw_file(name)
            .map(|slice| slice.to_vec())
            .with_context(|| format!("File '{}' not found in ADF", name))
    }
}

// ── Directory table scanner ──────────────────────────────────────────────────

fn scan_directory(disk_bytes: &[u8], base_offset: usize, out: &mut Vec<FileEntry>) {
    let mut pos = DIR_START;
    let mut raw_entries = Vec::new();

    while pos + 16 <= disk_bytes.len() {
        let chunk = &disk_bytes[pos..pos + 16];
        if chunk == [0xFF; 16] || chunk == [0x00; 16] {
            pos += 16;
            continue;
        }

        // Validate name bytes (first byte must be valid printable ASCII char)
        let first_b = chunk[0];
        if !(first_b.is_ascii_alphanumeric() || first_b == b'.' || first_b == b'_' || first_b == b'-') {
            break; // end of directory table
        }

        // Extract name (null-terminated / null-padded up to 10 bytes)
        let name_bytes = &chunk[..10];
        let null_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(10);
        let name = match std::str::from_utf8(&name_bytes[..null_pos]) {
            Ok(s) => s.trim().to_string(),
            Err(_) => {
                pos += 16;
                continue;
            }
        };

        if name.is_empty() {
            pos += 16;
            continue;
        }

        let flag = u16::from_be_bytes([chunk[10], chunk[11]]);
        let disk_off = u32::from_be_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]) as usize;

        if disk_off < disk_bytes.len() {
            raw_entries.push((name, flag, disk_off));
        }

        pos += 16;
    }

    // Sort entries by disk offset to calculate each file's stored length
    let mut sorted_indices: Vec<usize> = (0..raw_entries.len()).collect();
    sorted_indices.sort_by_key(|&idx| raw_entries[idx].2);

    for (rank, &orig_idx) in sorted_indices.iter().enumerate() {
        let (ref name, flag, disk_off) = raw_entries[orig_idx];
        let next_off = if rank + 1 < sorted_indices.len() {
            raw_entries[sorted_indices[rank + 1]].2
        } else {
            // Last file extends up to end of files section / track data
            // On Disk 1, files section ends at 0x97C00 (start of leveldata section)
            // On Disk 2, files extend to disk end
            if disk_bytes.len() > 0x97C00 && disk_off < 0x97C00 {
                0x97C00
            } else {
                disk_bytes.len()
            }
        };

        let size = next_off.saturating_sub(disk_off);

        out.push(FileEntry {
            name: name.clone(),
            offset: base_offset + disk_off,
            size,
            flag,
        });
    }
}
