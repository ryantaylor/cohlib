/// Discovery script: extracts files from CoH3 SGA archives and inspects their format.
/// Run with: cargo run --bin discover
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const DEPOT: &str = "/Users/ryantaylor/Library/Application Support/Steam/Steam.AppBundle/Steam/Contents/MacOS/steamapps/content/app_1677280/depot_1677281/anvil/archives";

const ARCHIVES: &[&str] = &[
    "ReferenceAttributes.sga",
    "LocaleEnglish.sga",
    // Skipping Attrib.sga (81MB) and Data.sga (28MB) for initial discovery
];

fn main() {
    let out_base = std::env::temp_dir().join("cohlib_discovery");

    for archive_name in ARCHIVES {
        let archive_path = format!("{DEPOT}/{archive_name}");
        let out_dir = out_base.join(archive_name.replace(".sga", ""));

        println!("\n{}", "=".repeat(60));
        println!("Archive: {archive_name}");
        println!("{}", "=".repeat(60));

        if out_dir.exists() {
            println!("  (using cached extraction at {})", out_dir.display());
        } else {
            println!("  Extracting to {}...", out_dir.display());
            fs::create_dir_all(&out_dir).expect("failed to create output dir");
            let out_str = out_dir.to_string_lossy().to_string();
            if let Err(e) = relic_sga::extract_all(&archive_path, &out_str) {
                println!("  ERROR extracting: {e}");
                continue;
            }
        }

        // Walk extracted files
        let mut all_files: Vec<_> = walkdir(&out_dir);
        all_files.sort();
        println!("  Total files: {}", all_files.len());

        // Extension summary
        let mut ext_counts: HashMap<String, usize> = HashMap::new();
        for f in &all_files {
            let ext = Path::new(f)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("(none)")
                .to_lowercase();
            *ext_counts.entry(ext).or_default() += 1;
        }
        let mut exts: Vec<_> = ext_counts.iter().collect();
        exts.sort_by(|a, b| b.1.cmp(a.1));
        println!("  Extensions:");
        for (ext, count) in &exts {
            println!("    .{ext:<20} {count}");
        }

        // Inspect first 10 files
        println!("\n  First 10 files:");
        for path in all_files.iter().take(10) {
            let rel = Path::new(path)
                .strip_prefix(&out_dir)
                .unwrap_or(Path::new(path))
                .display()
                .to_string();
            let data = match fs::read(path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let size = data.len();
            let magic_hex: String = data
                .iter()
                .take(16)
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            let magic_ascii: String = data
                .iter()
                .take(16)
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();

            println!("    {rel} ({size} bytes)");
            println!("      hex:   {magic_hex}");
            println!("      ascii: {magic_ascii}");

            // If small enough and looks like text, show content
            if size < 4096 {
                if let Ok(text) = std::str::from_utf8(&data) {
                    let preview: String = text.lines().take(3).collect::<Vec<_>>().join(" | ");
                    println!("      text:  {preview}");
                }
            }
        }
    }

    println!(
        "\n\nFull file listing saved. Extracted to: {}",
        out_base.display()
    );

    // Print all files for reference
    for archive_name in ARCHIVES {
        let out_dir = out_base.join(archive_name.replace(".sga", ""));
        if !out_dir.exists() {
            continue;
        }
        println!("\n--- {} all files ---", archive_name);
        let mut files = walkdir(&out_dir);
        files.sort();
        for f in &files {
            let rel = Path::new(f)
                .strip_prefix(&out_dir)
                .unwrap_or(Path::new(f))
                .display()
                .to_string();
            let size = fs::metadata(f).map(|m| m.len()).unwrap_or(0);
            println!("  {rel:<80} {size:>10} bytes");
        }
    }
}

fn walkdir(dir: &Path) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(walkdir(&path));
            } else {
                results.push(path.to_string_lossy().to_string());
            }
        }
    }
    results
}
