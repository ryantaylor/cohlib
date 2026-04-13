mod error;
pub use error::Error;

use std::{
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, Mutex},
};

use relic_sga::{
    entires::SgaEntries,
    nodes::{FolderNode, Toc},
};

/// A single file extracted from an SGA archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Normalized path of the file within the archive, using forward slashes.
    ///
    /// Example: `"instances/abilities/races/american/auto_build/auto_build_barracks.xml"`
    pub path: String,
    /// Raw bytes of the file.
    pub bytes: Vec<u8>,
}

impl ArchiveEntry {
    /// Returns the file extension, if any.
    pub fn extension(&self) -> Option<&str> {
        Path::new(&self.path).extension().and_then(|e| e.to_str())
    }
}

/// Opens an SGA archive at `path` and returns all contained files.
///
/// Archive paths are normalized to forward slashes. The SGA format stores folder
/// names as full root-relative Windows paths (e.g. `instances\abilities\races`),
/// so each file's path is built as `normalize(folder_name)/filename`.
pub fn open_archive(path: &Path) -> Result<Vec<ArchiveEntry>, Error> {
    let file =
        File::open(path).map_err(|e| Error::Sga(format!("cannot open {}: {e}", path.display())))?;
    let mut reader = BufReader::new(file);

    let mut entries = SgaEntries::new(&mut reader)
        .map_err(|e| Error::Sga(format!("cannot read SGA header: {e}")))?;

    let toc_entries = std::mem::take(&mut entries.tocs);
    let tocs: Vec<Toc> = toc_entries
        .into_iter()
        .map(|toc| {
            Toc::initialize_from_entry(&mut reader, &entries, toc)
                .map_err(|e| Error::Sga(format!("cannot initialize TOC: {e}")))
        })
        .collect::<Result<_, _>>()?;

    let mut results = Vec::new();
    for toc in &tocs {
        collect_files(&mut reader, toc.root_folder.clone(), &entries, &mut results)?;
    }

    Ok(results)
}

/// Recursively collects all files from a folder tree into `out`.
///
/// Each file's path is built from the folder's root-relative name plus the filename.
fn collect_files<R>(
    reader: &mut R,
    folder: Arc<Mutex<FolderNode>>,
    entries: &SgaEntries,
    out: &mut Vec<ArchiveEntry>,
) -> Result<(), Error>
where
    R: std::io::Read + std::io::BufRead + std::io::Seek,
{
    let folder_name = folder.lock().unwrap().name.clone();
    let folder_prefix = normalize_path(&folder_name);

    let files = FolderNode::read_files_from_folder(folder.clone(), reader, entries)
        .map_err(|e| Error::Sga(format!("cannot read files from '{folder_name}': {e}")))?;

    for file in files {
        let data = file
            .read_data(reader)
            .map_err(|e| Error::Sga(format!("cannot read file '{}': {e}", file.name)))?;

        let path = if folder_prefix.is_empty() {
            file.name.clone()
        } else {
            format!("{}/{}", folder_prefix, file.name)
        };

        out.push(ArchiveEntry { path, bytes: data });
    }

    let subfolders = FolderNode::read_folders_from_folder(folder.clone(), reader, entries)
        .map_err(|e| Error::Sga(format!("cannot read subfolders of '{folder_name}': {e}")))?;

    for subfolder in subfolders {
        collect_files(reader, Arc::new(Mutex::new(subfolder)), entries, out)?;
    }

    Ok(())
}

/// Reads the NiceName field from an SGA archive header.
///
/// The SGA v10 header layout (all fields little-endian):
///   0x00  8 bytes  magic "_ARCHIVE"
///   0x08  2 bytes  version (u16)
///   0x0A  2 bytes  product (u16)
///   0x0C  128 bytes NiceName (64 UTF-16 LE chars, null-padded)
///
/// Used to derive the per-archive AES key via RandomizeKey.
pub fn read_archive_name(path: &Path) -> Result<String, Error> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|e| Error::Sga(format!("cannot open {}: {e}", path.display())))?;
    let mut header = [0u8; 140]; // 8 magic + 2 version + 2 product + 128 name
    file.read_exact(&mut header)
        .map_err(|e| Error::Sga(format!("cannot read SGA header: {e}")))?;
    if &header[0..8] != b"_ARCHIVE" {
        return Err(Error::Sga("not an SGA archive".into()));
    }
    let name_bytes = &header[12..140];
    let utf16: Vec<u16> = name_bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&w| w != 0)
        .collect();
    String::from_utf16(&utf16).map_err(|e| Error::Sga(format!("invalid NiceName UTF-16: {e}")))
}

/// Normalizes a Windows path to forward slashes.
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_converts_backslashes() {
        assert_eq!(
            normalize_path(r"instances\abilities\races\american"),
            "instances/abilities/races/american"
        );
    }

    #[test]
    fn normalize_path_leaves_forward_slashes_unchanged() {
        assert_eq!(
            normalize_path("instances/abilities/foo.xml"),
            "instances/abilities/foo.xml"
        );
    }

    #[test]
    fn normalize_path_empty() {
        assert_eq!(normalize_path(""), "");
    }

    #[test]
    fn archive_entry_extension_xml() {
        let entry = ArchiveEntry {
            path: "instances/abilities/auto_build_barracks.xml".to_string(),
            bytes: vec![],
        };
        assert_eq!(entry.extension(), Some("xml"));
    }

    #[test]
    fn archive_entry_extension_none() {
        let entry = ArchiveEntry {
            path: "instances/abilities/foo".to_string(),
            bytes: vec![],
        };
        assert_eq!(entry.extension(), None);
    }
}
