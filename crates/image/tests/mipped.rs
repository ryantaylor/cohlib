/// Integration tests for mipped RRTEX extraction.
///
/// Fixture files were extracted from `anvil/archives/UI.sga` and
/// `anvil/archives/ScenariosMP.sga` in the CoH3 depot.
///
/// Texture variants encountered:
/// - Hybrid: small mips raw, large mips as sequential zlib streams
///   (american_mipped, italian_infantry_ak_icon_mipped)
/// - Pure zlib: all mips stored as sequential zlib streams, smallest first
///   (german_mipped, spec_ops_us_icon_mipped, special_weapons_us_icon_mipped,
///    indian_artillery_uk_icon_mipped)
/// - Non-mipped (mip_count=1): large textures split across many 64 KiB zlib streams
///   (semois map minimaps)

const AMERICAN: &[u8] = include_bytes!("fixtures/american_mipped.rrtex");
const ITALIAN_INFANTRY_AK: &[u8] =
    include_bytes!("fixtures/italian_infantry_ak_icon_mipped.rrtex");
const GERMAN: &[u8] = include_bytes!("fixtures/german_mipped.rrtex");
const SPEC_OPS_US: &[u8] = include_bytes!("fixtures/spec_ops_us_icon_mipped.rrtex");
const SPECIAL_WEAPONS_US: &[u8] =
    include_bytes!("fixtures/special_weapons_us_icon_mipped.rrtex");
const INDIAN_ARTILLERY_UK: &[u8] =
    include_bytes!("fixtures/indian_artillery_uk_icon_mipped.rrtex");
const SEMOIS_2P: &[u8] = include_bytes!("fixtures/semois_2p_mm_handmade.rrtex");
const SEMOIS_4P: &[u8] = include_bytes!("fixtures/semois_4p_mm_handmade.rrtex");

// ---------------------------------------------------------------------------
// Hybrid layout: small mips raw, large mips zlib-compressed
// ---------------------------------------------------------------------------

#[test]
fn american_mipped_extracts_without_error() {
    image::extract_icon(AMERICAN).expect("american_mipped.rrtex should extract successfully");
}

#[test]
fn american_mipped_produces_correct_dimensions() {
    let webp = image::extract_icon(AMERICAN).unwrap();
    let img = webp::Decoder::new(&webp)
        .decode()
        .expect("WebP should be decodable");
    assert_eq!(img.width(), 256, "expected width 256");
    assert_eq!(img.height(), 256, "expected height 256");
}

#[test]
fn italian_infantry_ak_icon_mipped_extracts_without_error() {
    image::extract_icon(ITALIAN_INFANTRY_AK)
        .expect("italian_infantry_ak_icon_mipped.rrtex should extract successfully");
}

#[test]
fn italian_infantry_ak_icon_mipped_produces_correct_dimensions() {
    let webp = image::extract_icon(ITALIAN_INFANTRY_AK).unwrap();
    let img = webp::Decoder::new(&webp)
        .decode()
        .expect("WebP should be decodable");
    assert_eq!(img.width(), 128, "expected width 128");
    assert_eq!(img.height(), 128, "expected height 128");
}

// ---------------------------------------------------------------------------
// Pure zlib layout: all mips stored as sequential zlib streams
// ---------------------------------------------------------------------------

#[test]
fn german_mipped_extracts_without_error() {
    image::extract_icon(GERMAN).expect("german_mipped.rrtex should extract successfully");
}

#[test]
fn german_mipped_produces_correct_dimensions() {
    let webp = image::extract_icon(GERMAN).unwrap();
    let img = webp::Decoder::new(&webp)
        .decode()
        .expect("WebP should be decodable");
    assert_eq!(img.width(), 256, "expected width 256");
    assert_eq!(img.height(), 256, "expected height 256");
}

#[test]
fn spec_ops_us_icon_mipped_extracts_without_error() {
    image::extract_icon(SPEC_OPS_US)
        .expect("spec_ops_us_icon_mipped.rrtex should extract successfully");
}

#[test]
fn special_weapons_us_icon_mipped_extracts_without_error() {
    image::extract_icon(SPECIAL_WEAPONS_US)
        .expect("special_weapons_us_icon_mipped.rrtex should extract successfully");
}

#[test]
fn indian_artillery_uk_icon_mipped_extracts_without_error() {
    image::extract_icon(INDIAN_ARTILLERY_UK)
        .expect("indian_artillery_uk_icon_mipped.rrtex should extract successfully");
}

// ---------------------------------------------------------------------------
// Non-mipped large texture: single logical mip split across many zlib streams
// ---------------------------------------------------------------------------

#[test]
fn semois_2p_mm_handmade_extracts_without_error() {
    image::extract_icon(SEMOIS_2P)
        .expect("semois_2p_mm_handmade.rrtex should extract successfully");
}

#[test]
fn semois_2p_mm_handmade_produces_correct_dimensions() {
    let webp = image::extract_icon(SEMOIS_2P).unwrap();
    let img = webp::Decoder::new(&webp)
        .decode()
        .expect("WebP should be decodable");
    assert_eq!(img.width(), 2048, "expected width 2048");
    assert_eq!(img.height(), 2048, "expected height 2048");
}

#[test]
fn semois_4p_mm_handmade_extracts_without_error() {
    image::extract_icon(SEMOIS_4P)
        .expect("semois_4p_mm_handmade.rrtex should extract successfully");
}
