//! Static derivation of the marketing semver string (`major.minor.patch`) from
//! `RelicCoH3.exe` bytes alone — no game launch or runtime memory access.
//!
//! The build number (the fourth segment of the full `major.minor.patch.build`
//! string) is extracted separately by `read_exe_version` in `main.rs` and is
//! the same value used as `GameData::version`.
//!
//! See the top-level `derive_semver.py` / `derive_semver.ps1` reference scripts
//! for the full reverse-engineering notes.
//!
//! Algorithm:
//! 1. Find UTF-16LE format string `L"%d.%d.%d.%d\0"` in `.rdata`.
//! 2. Scan `.text` for the byte signature:
//!    ```text
//!    41 B9 mm 00 00 00       mov r9d, <minor>
//!    41 B8 MM 00 00 00       mov r8d, <major>
//!    48 8D 15 dd dd dd dd    lea rdx, [rip+disp32]  ; disp resolves to fmt string
//!    ```
//! 3. Extract `major` and `minor` from those immediates.
//! 4. Identify the patch immediate. `patch` is the 5th swprintf argument,
//!    passed via `[rsp+0x20]`. The stack store appears immediately before the
//!    `mov r9d, <minor>`. The compiler's choice of source register varies
//!    across builds (r13 in 2.4.1, r15 in 2.4.0). We:
//!    (a) Decode the `mov [rsp+0x20], <reg>` instruction immediately preceding
//!    the marker to identify the source register, OR
//!    (b) Detect a `mov dword [rsp+0x20], imm32` (`c7 44 24 20 ?? ?? ?? ??`)
//!    which stores the literal directly without a register.
//!    Then scan backward up to 0x800 bytes for that register's latest
//!    assignment (`mov reg, imm32` or `xor reg, reg` for zero).

use data::Semver;
use std::path::Path;

/// Reads `exe_path` and derives the marketing `Semver { major, minor, patch }`.
pub fn derive_semver(exe_path: &Path) -> Result<Semver, String> {
    let buf = std::fs::read(exe_path)
        .map_err(|e| format!("cannot read {}: {e}", exe_path.display()))?;
    derive_from_bytes(&buf)
}

fn derive_from_bytes(buf: &[u8]) -> Result<Semver, String> {
    let (image_base, sections) = parse_pe_sections(buf)?;
    let fmt_rva = find_format_string_rva(buf, &sections)?;
    let text = find_section(&sections, ".text")
        .ok_or_else(|| ".text section not found".to_string())?;

    let (site_off, major, minor) = find_marketing_swprintf(buf, text, image_base, fmt_rva)?;
    let patch = find_patch_immediate(buf, text, site_off)?;
    Ok(Semver { major, minor, patch })
}

struct Section {
    name: String,
    va: u32,
    ro: u32,
    rs: u32,
}

fn parse_pe_sections(buf: &[u8]) -> Result<(u64, Vec<Section>), String> {
    if buf.len() < 0x40 {
        return Err("buffer too small to be a PE file".into());
    }
    let pe_off = u32::from_le_bytes(buf[0x3C..0x40].try_into().unwrap()) as usize;
    if pe_off + 24 > buf.len() {
        return Err("PE header offset out of range".into());
    }
    let num_sec = u16::from_le_bytes(buf[pe_off + 6..pe_off + 8].try_into().unwrap()) as usize;
    let opt_size =
        u16::from_le_bytes(buf[pe_off + 20..pe_off + 22].try_into().unwrap()) as usize;
    let opt_base = pe_off + 24;
    if opt_base + 32 > buf.len() {
        return Err("optional header truncated".into());
    }
    let image_base = u64::from_le_bytes(buf[opt_base + 24..opt_base + 32].try_into().unwrap());
    let sec_tbl = opt_base + opt_size;

    let mut sections = Vec::with_capacity(num_sec);
    for i in 0..num_sec {
        let b = sec_tbl + i * 40;
        if b + 40 > buf.len() {
            return Err("section table truncated".into());
        }
        let name = String::from_utf8_lossy(
            buf[b..b + 8].split(|&c| c == 0).next().unwrap_or(&[]),
        )
        .into_owned();
        let va = u32::from_le_bytes(buf[b + 12..b + 16].try_into().unwrap());
        let rs = u32::from_le_bytes(buf[b + 16..b + 20].try_into().unwrap());
        let ro = u32::from_le_bytes(buf[b + 20..b + 24].try_into().unwrap());
        sections.push(Section { name, va, ro, rs });
    }
    Ok((image_base, sections))
}

fn find_section<'a>(sections: &'a [Section], name: &str) -> Option<&'a Section> {
    sections.iter().find(|s| s.name == name)
}

fn find_format_string_rva(buf: &[u8], sections: &[Section]) -> Result<u32, String> {
    let rdata = find_section(sections, ".rdata")
        .ok_or_else(|| ".rdata section not found".to_string())?;
    // UTF-16LE "%d.%d.%d.%d\0"
    let needle: Vec<u8> = "%d.%d.%d.%d\0"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let region_start = rdata.ro as usize;
    let region_end = region_start + rdata.rs as usize;
    let region = buf
        .get(region_start..region_end)
        .ok_or_else(|| ".rdata region out of range".to_string())?;
    let idx = find_subslice(region, &needle)
        .ok_or_else(|| r#"L"%d.%d.%d.%d" not found in .rdata"#.to_string())?;
    Ok(rdata.va + idx as u32)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Locates the marketing-version swprintf site and returns
/// `(file_offset_of_site, major, minor)`. Prefers candidates whose major != 5
/// (the PE-style swprintf uses major=5).
fn find_marketing_swprintf(
    buf: &[u8],
    text: &Section,
    image_base: u64,
    fmt_rva: u32,
) -> Result<(usize, u8, u8), String> {
    let text_ro = text.ro as usize;
    let text_rs = text.rs as usize;
    if text_ro + text_rs > buf.len() {
        return Err(".text section extends past end of file".into());
    }
    let end = text_ro + text_rs - 19;

    let mut candidates: Vec<(usize, u8, u8)> = Vec::new();
    let mut i = text_ro;
    while i <= end {
        // 41 B9 mm 00 00 00 41 B8 MM 00 00 00 48 8D 15 dd dd dd dd
        if buf[i] == 0x41
            && buf[i + 1] == 0xB9
            && buf[i + 3] == 0x00
            && buf[i + 4] == 0x00
            && buf[i + 5] == 0x00
            && buf[i + 6] == 0x41
            && buf[i + 7] == 0xB8
            && buf[i + 9] == 0x00
            && buf[i + 10] == 0x00
            && buf[i + 11] == 0x00
            && buf[i + 12] == 0x48
            && buf[i + 13] == 0x8D
            && buf[i + 14] == 0x15
        {
            let minor = buf[i + 2];
            let major = buf[i + 8];
            let disp32 = i32::from_le_bytes(buf[i + 15..i + 19].try_into().unwrap());
            let lea_file_off = i + 12;
            let lea_instr_va =
                image_base + text.va as u64 + (lea_file_off - text_ro) as u64;
            let lea_target = (lea_instr_va as i64) + 7 + disp32 as i64;
            let target_rva = lea_target - image_base as i64;
            if target_rva == fmt_rva as i64 {
                candidates.push((i, major, minor));
            }
        }
        i += 1;
    }

    if candidates.is_empty() {
        return Err("marketing-version swprintf pattern not found".into());
    }
    // Prefer candidates whose major != 5 (filter out the PE-style swprintf).
    let chosen = candidates
        .iter()
        .find(|c| c.1 != 5)
        .copied()
        .unwrap_or(candidates[0]);
    Ok(chosen)
}

/// The 5th swprintf arg (`patch`) is passed via `[rsp+0x20]`. The stack store
/// appears immediately before the `mov r9d, <minor>` at `site_off`. We decode
/// it to identify the source register, then walk backward for that register's
/// assignment. Alternatively, the compiler may store the literal directly via
/// `c7 44 24 20 imm32`.
fn find_patch_immediate(buf: &[u8], text: &Section, site_off: usize) -> Result<u8, String> {
    let text_ro = text.ro as usize;

    // Case A: `mov dword [rsp+0x20], imm32` — literal stored directly.
    //   c7 44 24 20 ii 00 00 00
    if site_off >= 8 {
        let p = site_off - 8;
        if buf[p] == 0xC7
            && buf[p + 1] == 0x44
            && buf[p + 2] == 0x24
            && buf[p + 3] == 0x20
            && buf[p + 5] == 0
            && buf[p + 6] == 0
            && buf[p + 7] == 0
        {
            return Ok(buf[p + 4]);
        }
    }

    // Case B: `mov [rsp+0x20], <reg>` — 5-byte form `<rex> 89 <modrm> 24 20`.
    let reg = if site_off >= 5
        && buf[site_off - 4] == 0x89
        && buf[site_off - 2] == 0x24
        && buf[site_off - 1] == 0x20
    {
        decode_rsp20_store_reg(buf[site_off - 5], buf[site_off - 3])
    } else {
        None
    };

    let reg = reg.ok_or_else(|| {
        format!(
            "no recognized `mov [rsp+0x20], <reg>` or `mov dword [rsp+0x20], imm` immediately \
             before marketing-version setup at .text+0x{:X}",
            site_off - text_ro
        )
    })?;

    // Scan back up to 0x800 bytes for `<reg>` assignment:
    //   mov reg32, imm32           e.g. r13d: 41 BD ?? 00 00 00 | eax: B8 ?? 00 00 00
    //   xor reg32, reg32 (= 0)     e.g. r13d: 45 33 ED        | eax: 33 C0
    let scan_start = site_off.saturating_sub(0x800).max(text_ro);
    let mut j = site_off.saturating_sub(6);
    while j >= scan_start {
        if let Some(imm) = match_reg_assignment(buf, j, reg) {
            return Ok(imm);
        }
        if j == scan_start {
            break;
        }
        j -= 1;
    }
    Err(format!(
        "could not locate assignment to {} within 0x800 bytes before site at .text+0x{:X}",
        reg_name(reg),
        site_off - text_ro
    ))
}

/// Decode the source register of `<rex> 89 <modrm> 24 20`.
/// modrm = `(mod=01)(reg=src&7)(rm=100)` = `0x40 | (src&7) << 3 | 0x04` = `0x44 | (src&7)<<3`.
/// REX.R (rex byte bit 2 = 0x04) extends the source register to r8..r15.
/// Returns a register index 0..15, or None if the bytes don't decode.
fn decode_rsp20_store_reg(rex: u8, modrm: u8) -> Option<u8> {
    // High nibble of rex must be 0x4
    if rex & 0xF0 != 0x40 {
        return None;
    }
    // modrm: mod=01 (top two bits), rm=100 (low three bits) → mask 0xC7 == 0x44
    if modrm & 0xC7 != 0x44 {
        return None;
    }
    let reg_lo = (modrm >> 3) & 0x07;
    let rex_r = (rex >> 2) & 0x01;
    Some((rex_r << 3) | reg_lo)
}

fn reg_name(reg: u8) -> &'static str {
    match reg {
        0 => "eax", 1 => "ecx", 2 => "edx", 3 => "ebx",
        4 => "esp", 5 => "ebp", 6 => "esi", 7 => "edi",
        8 => "r8d", 9 => "r9d", 10 => "r10d", 11 => "r11d",
        12 => "r12d", 13 => "r13d", 14 => "r14d", 15 => "r15d",
        _ => "?",
    }
}

/// Returns the immediate value if bytes at `j` encode an assignment to `reg`.
fn match_reg_assignment(buf: &[u8], j: usize, reg: u8) -> Option<u8> {
    if reg < 8 {
        // No REX prefix needed for low 8 regs.
        // mov reg32, imm32: B8+reg ii ii ii ii (5 bytes)
        if j + 4 < buf.len() && buf[j] == 0xB8 + reg && buf[j + 2] == 0 && buf[j + 3] == 0 && buf[j + 4] == 0 {
            return Some(buf[j + 1]);
        }
        // xor reg32, reg32: 33 (mod=11, reg=reg, rm=reg) — modrm = 0xC0 | reg<<3 | reg
        if j + 1 < buf.len() && buf[j] == 0x33 && buf[j + 1] == 0xC0 | (reg << 3) | reg {
            return Some(0);
        }
    } else {
        // REX.B needed to extend rm side; for mov reg32,imm32 with extended reg: 41 B8+(reg-8) imm32 (6 bytes)
        let lo = reg - 8;
        if j + 5 < buf.len()
            && buf[j] == 0x41
            && buf[j + 1] == 0xB8 + lo
            && buf[j + 3] == 0
            && buf[j + 4] == 0
            && buf[j + 5] == 0
        {
            return Some(buf[j + 2]);
        }
        // xor r??d, r??d: 45 33 (modrm: 0xC0 | lo<<3 | lo)
        if j + 2 < buf.len()
            && buf[j] == 0x45
            && buf[j + 1] == 0x33
            && buf[j + 2] == 0xC0 | (lo << 3) | lo
        {
            return Some(0);
        }
    }
    None
}
