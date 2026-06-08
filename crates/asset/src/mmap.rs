use std::fs::File;
use std::ops::Deref;
use std::path::Path;

/// Minimum file size (in bytes) to trigger memory mapping instead of heap read.
const MMAP_THRESHOLD: u64 = 64 * 1024;

/// A file-backed byte buffer, either memory-mapped or heap-allocated.
pub struct MappedFile {
    // Either mmap or heap-allocated bytes.  Only one is populated.
    pub mmap: Option<memmap2::Mmap>,
    pub heap: Option<Vec<u8>>,
    path: Option<std::path::PathBuf>,
}

impl MappedFile {
    /// Open the file at `path` and load its contents.
    ///
    /// - Files >= `MMAP_THRESHOLD` are memory-mapped read-only.
    /// - Smaller files are read into a `Vec<u8>`.
    pub fn open(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("failed to stat {}: {}", path.display(), e))?;
        let len = metadata.len();

        if len >= MMAP_THRESHOLD {
            let file = File::open(path)
                .map_err(|e| format!("failed to open {}: {}", path.display(), e))?;
            let mmap = unsafe {
                memmap2::Mmap::map(&file)
                    .map_err(|e| format!("failed to mmap {}: {}", path.display(), e))?
            };
            tracing::debug!(
                path = %path.display(),
                bytes = len,
                "memory-mapped asset file"
            );
            Ok(Self {
                mmap: Some(mmap),
                heap: None,
                path: Some(path.to_path_buf()),
            })
        } else {
            let bytes = std::fs::read(path)
                .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
            tracing::debug!(
                path = %path.display(),
                bytes = len,
                "read asset file into heap"
            );
            Ok(Self {
                mmap: None,
                heap: Some(bytes),
                path: Some(path.to_path_buf()),
            })
        }
    }

    /// Always memory-map the file, even if it is small.
    pub fn open_mapped(path: &Path) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|e| format!("failed to open {}: {}", path.display(), e))?;
        let mmap = unsafe {
            memmap2::Mmap::map(&file)
                .map_err(|e| format!("failed to mmap {}: {}", path.display(), e))?
        };
        Ok(Self {
            mmap: Some(mmap),
            heap: None,
            path: Some(path.to_path_buf()),
        })
    }

    /// Always read the file into a heap `Vec<u8>`, bypassing mmap.
    pub fn open_heap(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
        Ok(Self {
            mmap: None,
            heap: Some(bytes),
            path: Some(path.to_path_buf()),
        })
    }

    /// File path, if known.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl Deref for MappedFile {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match (&self.mmap, &self.heap) {
            (Some(mmap), None) => mmap.as_ref(),
            (None, Some(heap)) => heap.as_slice(),
            _ => &[],
        }
    }
}

/// Convenience: load file bytes, auto-selecting mmap for large files.
pub fn load_file_bytes(path: &Path) -> Result<MappedFile, String> {
    MappedFile::open(path)
}

/// Convenience: load file bytes synchronously, returning a `Vec<u8>`.
/// This is useful when the caller needs ownership of the bytes.
pub fn load_file_bytes_vec(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("failed to read {}: {}", path.display(), e))
}
