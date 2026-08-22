use std::cmp::Ordering;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use zip::read::ZipArchive;

use crate::types::{ClipboardItem, ClipboardMode, EntryItem};

// ============================================================================
// Operace s adresáři - list_entries, volumes, drive handling
// ============================================================================

/// Vyjmenování obsahu adresáře - složky a soubory
pub fn list_entries(current_dir: &Path) -> io::Result<Vec<EntryItem>> {
    let mut entries: Vec<_> = fs::read_dir(current_dir)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            let file_type = entry.file_type().ok();

            EntryItem {
                name: entry.file_name().to_string_lossy().into_owned(),
                path,
                is_dir: file_type.as_ref().is_some_and(|kind| kind.is_dir()),
                is_file: file_type.as_ref().is_some_and(|kind| kind.is_file()),
            }
        })
        .collect();

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(entries)
}

/// Vytvoření cesty ke kořeni disku - např. "C:\"
#[cfg(target_os = "windows")]
fn drive_root(letter: char) -> PathBuf {
    PathBuf::from(format!("{}:\\", letter.to_ascii_uppercase()))
}

/// Zjištění dostupných disků
#[cfg(target_os = "windows")]
pub fn available_volumes() -> Vec<PathBuf> {
    let mut volumes = Vec::new();

    for letter in 'C'..='Z' {
        let root = drive_root(letter);
        if root.exists() {
            volumes.push(root);
        }
    }

    volumes
}

/// Zjištění dostupných disků
#[cfg(target_os = "linux")]
pub fn available_volumes() -> Vec<PathBuf> {
    let mut volumes = linux_mount_points();
    if volumes.is_empty() {
        add_root_if_exists(&mut volumes, Path::new("/"));
    }

    for base in ["/mnt", "/media"] {
        collect_root_directories(&mut volumes, Path::new(base), 2);
    }

    volumes.sort_by(|left, right| left.display().to_string().cmp(&right.display().to_string()));
    volumes.dedup();
    volumes
}

/// Zjištění dostupných disků
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn available_volumes() -> Vec<PathBuf> {
    let mut volumes = Vec::new();
    add_root_if_exists(&mut volumes, Path::new("/"));

    for base in ["/mnt", "/media", "/Volumes"] {
        collect_root_directories(&mut volumes, Path::new(base), 2);
    }

    volumes.sort_by(|left, right| left.display().to_string().cmp(&right.display().to_string()));
    volumes.dedup();
    volumes
}

#[cfg(target_os = "linux")]
fn linux_mount_points() -> Vec<PathBuf> {
    let mut volumes = Vec::new();

    let mounts_data = fs::read_to_string("/proc/mounts")
        .or_else(|_| fs::read_to_string("/etc/mtab"));

    let Ok(mounts_data) = mounts_data else {
        return volumes;
    };

    for line in mounts_data.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Some((mount_point, fs_type)) = parse_mount_line(line) {
            if is_virtual_fs(fs_type) {
                continue;
            }

            if mount_point.is_dir() {
                add_root_if_exists(&mut volumes, &mount_point);
            }
        }
    }

    add_root_if_exists(&mut volumes, Path::new("/"));
    volumes
}

#[cfg(target_os = "linux")]
fn parse_mount_line(line: &str) -> Option<(PathBuf, &str)> {
    let mut parts = line.split_whitespace();
    let _device = parts.next()?;
    let mount_point_raw = parts.next()?;
    let fs_type = parts.next()?;

    let mount_point = unescape_mount_field(mount_point_raw);
    let mount_path = PathBuf::from(mount_point);
    if !mount_path.is_absolute() {
        return None;
    }

    Some((mount_path, fs_type))
}

#[cfg(target_os = "linux")]
fn unescape_mount_field(value: &str) -> String {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\012", "\n")
        .replace("\\134", "\\")
}

#[cfg(target_os = "linux")]
fn is_virtual_fs(fs_type: &str) -> bool {
    matches!(
        fs_type,
        "proc"
            | "sysfs"
            | "tmpfs"
            | "devtmpfs"
            | "devpts"
            | "cgroup"
            | "cgroup2"
            | "securityfs"
            | "pstore"
            | "bpf"
            | "mqueue"
            | "hugetlbfs"
            | "tracefs"
            | "fusectl"
            | "configfs"
            | "debugfs"
            | "overlay"
            | "nsfs"
            | "rpc_pipefs"
            | "autofs"
            | "binfmt_misc"
    )
}

#[cfg(not(target_os = "windows"))]
fn add_root_if_exists(volumes: &mut Vec<PathBuf>, path: &Path) {
    if path.exists() && !volumes.iter().any(|existing| existing == path) {
        volumes.push(path.to_path_buf());
    }
}

#[cfg(not(target_os = "windows"))]
fn collect_root_directories(volumes: &mut Vec<PathBuf>, base: &Path, depth: usize) {
    if depth == 0 || !base.exists() || !base.is_dir() {
        return;
    }

    add_root_if_exists(volumes, base);

    if depth == 1 {
        return;
    }

    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_root_directories(volumes, &path, depth.saturating_sub(1));
            }
        }
    }
}

/// Formátování názvu disku pro zobrazení - "1: C:\" atd.
pub fn volume_label(index: usize, path: &Path) -> String {
    let display = path.display().to_string();
    format!("{}: {}", index + 1, display)
}

// ============================================================================
// Čtení preview - tekstové soubory a ZIP archivy
// ============================================================================

/// Čtení náhledu souboru - max_bytes znaků textu
pub fn read_preview(path: &Path, max_bytes: usize) -> io::Result<String> {
    if is_zip_file(path) {
        return read_zip_preview(path);
    }

    let bytes = fs::read(path)?;
    let slice = if bytes.len() > max_bytes {
        &bytes[..max_bytes]
    } else {
        &bytes
    };

    let mut text = sanitize_preview_text(&String::from_utf8_lossy(slice));
    if bytes.len() > max_bytes {
        text.push_str("\n... (zkráceno)");
    }

    Ok(text)
}

/// Sanitizace textu - nahrazení control znaků mezerami
fn sanitize_preview_text(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch == '\n' || ch == '\r' || ch == '\t' {
                ch
            } else if ch.is_control() {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

/// Zjištění, zda je soubor ZIP archivem
fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

/// Čtení obsahu ZIP archivu - seznam files
fn read_zip_preview(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;

    if archive.len() == 0 {
        return Ok(String::from("ZIP archive je prázdný."));
    }

    let mut lines = Vec::new();
    lines.push(format!("ZIP archiv: {}", path.display()));
    lines.push(String::new());

    for index in 0..archive.len().min(40) {
        let file = archive
            .by_index(index)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;

        if file.encrypted() {
            return Ok(String::from("Archiv je zašifrovaný."));
        }

        let kind = if file.is_dir() { "DIR" } else { "FILE" };
        lines.push(format!("[{kind}] {}", file.name()));
    }

    if archive.len() > 40 {
        lines.push(String::from("... (více zápisů)"));
    }

    Ok(lines.join("\n"))
}

// ============================================================================
// Operace se soubory - otevírání, mazání, kopírování, vkládání
// ============================================================================

/// Otevření souboru v platformově výchozí aplikaci.
#[cfg(target_os = "windows")]
pub fn open_with_default_app(path: &Path) -> io::Result<()> {
    Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Otevření souboru v platformově výchozí aplikaci.
#[cfg(target_os = "macos")]
pub fn open_with_default_app(path: &Path) -> io::Result<()> {
    Command::new("open").arg(path).spawn().map(|_| ())
}

/// Otevření souboru v platformově výchozí aplikaci.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn open_with_default_app(path: &Path) -> io::Result<()> {
    Command::new("xdg-open").arg(path).spawn().map(|_| ())
}

/// Smazání souboru nebo složky (s innehancem)
pub fn delete_path(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Vkládání ze schránky - zkopíruj nebo přesuň
pub fn paste_clipboard(item: &ClipboardItem, target_dir: &Path) -> io::Result<PathBuf> {
    let file_name = item
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("item");

    let destination = unique_destination(target_dir, file_name);

    match item.mode {
        ClipboardMode::Copy => {
            copy_recursively(&item.source, &destination)?;
        }
        ClipboardMode::Cut => {
            if fs::rename(&item.source, &destination).is_err() {
                copy_recursively(&item.source, &destination)?;
                delete_path(&item.source)?;
            }
        }
    }

    Ok(destination)
}

/// Rekurzívní kopírování - soubor nebo vlna složek
fn copy_recursively(source: &Path, destination: &Path) -> io::Result<()> {
    if source.is_file() {
        fs::copy(source, destination)?;
        return Ok(());
    }

    if source.is_dir() {
        fs::create_dir_all(destination)?;

        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let child_source = entry.path();
            let child_destination = destination.join(entry.file_name());
            copy_recursively(&child_source, &child_destination)?;
        }
    }

    Ok(())
}

/// Určení jedinečné cíle - pokud soubor existuje, přidej _copy1, _copy2, ...
fn unique_destination(target_dir: &Path, file_name: &str) -> PathBuf {
    let mut candidate = target_dir.join(file_name);

    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");

    for index in 1..1000 {
        let name = if extension.is_empty() {
            format!("{}_copy{}", stem, index)
        } else {
            format!("{}_copy{}.{}", stem, index, extension)
        };
        candidate = target_dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    target_dir.join(format!("{}_copy_final", stem))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn sanitize_preview_replaces_control_chars() {
        let text = sanitize_preview_text("a\u{0007}b\n\t");
        assert_eq!(text, "a b\n\t");
    }

    #[test]
    fn unique_destination_adds_copy_suffix() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let first = temp_dir.path().join("file.txt");
        fs::write(&first, b"one").expect("write");

        let destination = unique_destination(temp_dir.path(), "file.txt");

        assert_ne!(destination, first);
        assert!(destination.to_string_lossy().contains("_copy"));
    }

    #[test]
    fn list_entries_sorts_directories_first() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let folder = temp_dir.path().join("folder");
        let file = temp_dir.path().join("file.txt");
        fs::create_dir(&folder).expect("create dir");
        let mut handle = fs::File::create(&file).expect("create file");
        handle.write_all(b"hello").expect("write file");

        let entries = list_entries(temp_dir.path()).expect("list entries");

        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_dir);
        assert!(entries[1].is_file);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_mount_line_extracts_mount_path_and_fs() {
        let line = "/dev/sda2 /mnt/data ext4 rw,relatime 0 0";
        let parsed = parse_mount_line(line).expect("mount line");

        assert_eq!(parsed.0, PathBuf::from("/mnt/data"));
        assert_eq!(parsed.1, "ext4");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mount_unescape_decodes_spaces() {
        let unescaped = unescape_mount_field("/run/media/My\\040Disk");
        assert_eq!(unescaped, "/run/media/My Disk");
    }
}
