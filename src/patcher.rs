//! Disc handling: read a .cue, check Track 01, apply the embedded PPF patch and
//! write an English-named copy of the whole disc next to (or away from) the original.
//! Nothing here touches the user's original files.

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// The patch itself (PPF 3.0) - only our translated text, font and a few code bytes.
pub const PATCH: &[u8] = include_bytes!("../assets/patch.ppf");

/// Size of the Japanese Track 01 (data track), in bytes.
pub const TRACK1_SIZE: u64 = 15_772_512;
/// SHA-1 of an unmodified Japanese Track 01.
pub const ORIGINAL_SHA1: &str = "805bb006d4914d77b36a28ab5cb7393d50e9d73c";
/// SHA-1 of Track 01 after patching (used to verify our own output).
pub const PATCHED_SHA1: &str = "f43eeaa166ef5d78be8e7b387a6375f79dbf6946";

pub const PATCH_VERSION: &str = "v0.3";
pub const ENGLISH_BASE: &str =
    "Arthur & Astaroth's Mysterious Ghosts 'n Goblins - Incredible Toons (English)";

#[derive(Debug, Clone)]
pub struct CueFile {
    /// Path as written in the .cue
    pub name: String,
    /// Resolved path on disk
    pub path: PathBuf,
    /// Track numbers stored in this file
    pub tracks: Vec<u32>,
    /// Track modes (e.g. "MODE2/2352", "AUDIO") in the same order
    pub modes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Disc {
    pub cue_path: PathBuf,
    pub cue_text: String,
    pub files: Vec<CueFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Track1State {
    /// Exactly the Japanese original - safe to patch.
    Original,
    /// Already carries this English patch.
    AlreadyPatched,
    /// Some other dump/version; patching may not work.
    Unknown(String),
}

#[derive(Debug, Clone)]
pub struct Checked {
    pub disc: Disc,
    pub state: Track1State,
}

pub fn err<T>(msg: impl Into<String>) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::Other, msg.into()))
}

/// Accepts a .cue, or a .bin that a .cue in the same folder refers to.
pub fn resolve_cue(input: &Path) -> io::Result<PathBuf> {
    let ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "cue" => Ok(input.to_path_buf()),
        "bin" => {
            let dir = input.parent().unwrap_or(Path::new("."));
            let want = input.file_name().unwrap().to_string_lossy().to_lowercase();
            for entry in fs::read_dir(dir)? {
                let p = entry?.path();
                if p.extension()
                    .map(|e| e.eq_ignore_ascii_case("cue"))
                    .unwrap_or(false)
                {
                    if let Ok(text) = read_text(&p) {
                        if text.to_lowercase().contains(&want) {
                            return Ok(p);
                        }
                    }
                }
            }
            err("Couldn't find a .cue file for that .bin. Please choose the game's .cue file.")
        }
        "chd" => err(
            "CHD files aren't supported yet. Convert it back to .cue/.bin first \
             (chdman extractcd -i game.chd -o game.cue), then choose the .cue.",
        ),
        "iso" => err("An .iso can't hold this game's music tracks. Please use a .cue/.bin dump."),
        _ => err("Please choose the game's .cue file."),
    }
}

fn read_text(p: &Path) -> io::Result<String> {
    let bytes = fs::read(p)?;
    // .cue files are plain ASCII/Latin-1 in practice; be forgiving.
    Ok(bytes.iter().map(|&b| b as char).collect())
}

/// Splits a cue line into tokens, honouring "double quoted strings".
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    for ch in line.chars() {
        match ch {
            '"' => in_q = !in_q,
            c if c.is_whitespace() && !in_q => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn parse_cue(cue_path: &Path) -> io::Result<Disc> {
    let text = read_text(cue_path)?;
    let dir = cue_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut files: Vec<CueFile> = Vec::new();
    for line in text.lines() {
        let t = tokens(line.trim());
        if t.is_empty() {
            continue;
        }
        match t[0].to_uppercase().as_str() {
            "FILE" if t.len() >= 2 => {
                // FILE name may contain spaces if the cue was hand-written without quotes
                let name = if t.len() > 3 {
                    t[1..t.len() - 1].join(" ")
                } else {
                    t[1].clone()
                };
                files.push(CueFile {
                    path: dir.join(&name),
                    name,
                    tracks: vec![],
                    modes: vec![],
                });
            }
            "TRACK" if t.len() >= 3 => {
                let f = match files.last_mut() {
                    Some(f) => f,
                    None => return err("This .cue file looks damaged (TRACK before FILE)."),
                };
                f.tracks.push(t[1].parse().unwrap_or(0));
                f.modes.push(t[2].to_uppercase());
            }
            _ => {}
        }
    }
    if files.is_empty() {
        return err("That .cue file doesn't list any tracks.");
    }
    if files[0].tracks.first() != Some(&1)
        || files[0].modes.first().map(|m| m.as_str()) != Some("MODE2/2352")
    {
        return err(
            "This doesn't look like the right game: Track 01 should be a MODE2/2352 data track.",
        );
    }
    for f in &files {
        if !f.path.exists() {
            return err(format!("Missing file listed in the .cue:\n{}", f.name));
        }
    }
    let total_tracks: usize = files.iter().map(|f| f.tracks.len()).sum();
    if total_tracks != 19 {
        return err(format!(
            "This disc has {} tracks; Arthur to Astaroth no Nazo Makaimura has 19. \
             Is this the right game?",
            total_tracks
        ));
    }
    Ok(Disc {
        cue_path: cue_path.to_path_buf(),
        cue_text: text,
        files,
    })
}

pub fn sha1_hex(data: &[u8]) -> String {
    let mut h = sha1_smol::Sha1::new();
    h.update(data);
    h.digest().to_string()
}

/// Reads the first TRACK1_SIZE bytes of the file holding Track 01.
fn read_track1(disc: &Disc) -> io::Result<Vec<u8>> {
    let f = &disc.files[0];
    let len = fs::metadata(&f.path)?.len();
    if len < TRACK1_SIZE {
        return err("Track 01 is smaller than expected. The dump may be incomplete.");
    }
    let mut buf = vec![0u8; TRACK1_SIZE as usize];
    File::open(&f.path)?.read_exact(&mut buf)?;
    Ok(buf)
}

pub fn check(input: &Path) -> io::Result<Checked> {
    let cue = resolve_cue(input)?;
    let disc = parse_cue(&cue)?;
    let t1 = read_track1(&disc)?;
    let sum = sha1_hex(&t1);
    let state = if sum == ORIGINAL_SHA1 {
        Track1State::Original
    } else if sum == PATCHED_SHA1 {
        Track1State::AlreadyPatched
    } else {
        Track1State::Unknown(sum)
    };
    Ok(Checked { disc, state })
}

/// Applies a PPF 3.0 (or 2.0) patch to `data` in place.
pub fn apply_ppf(data: &mut [u8], ppf: &[u8]) -> io::Result<()> {
    if ppf.len() < 60 || &ppf[0..3] != b"PPF" {
        return err("Embedded patch is damaged.");
    }
    let (mut pos, offset_size) = match &ppf[3..5] {
        b"30" => {
            let blockcheck = ppf[57];
            let mut p = 60;
            if blockcheck != 0 {
                p += 1024;
            }
            (p, 8usize)
        }
        b"20" => (1084usize, 4usize),
        _ => return err("Unsupported patch format."),
    };
    let undo = ppf[3..5] == *b"30" && ppf[58] != 0;
    while pos < ppf.len() {
        // PPF3 may end with a "@BEGIN_FILE_ID.DIZ" block
        if ppf[pos..].starts_with(b"@BEG") {
            break;
        }
        if pos + offset_size + 1 > ppf.len() {
            return err("Embedded patch is truncated.");
        }
        let off = if offset_size == 8 {
            u64::from_le_bytes(ppf[pos..pos + 8].try_into().unwrap())
        } else {
            u32::from_le_bytes(ppf[pos..pos + 4].try_into().unwrap()) as u64
        } as usize;
        pos += offset_size;
        let n = ppf[pos] as usize;
        pos += 1;
        if pos + n > ppf.len() || off + n > data.len() {
            return err("Patch doesn't fit this file.");
        }
        data[off..off + n].copy_from_slice(&ppf[pos..pos + n]);
        pos += n;
        if undo {
            pos += n;
        }
    }
    Ok(())
}

pub fn english_name(index: usize, total_files: usize) -> String {
    if total_files == 1 {
        format!("{}.bin", ENGLISH_BASE)
    } else {
        format!("{} (Track {:02}).bin", ENGLISH_BASE, index + 1)
    }
}

/// Builds the new .cue text: same layout, FILE lines renamed.
pub fn english_cue(disc: &Disc) -> String {
    let mut out = String::new();
    let mut k = 0usize;
    for line in disc.cue_text.lines() {
        let t = tokens(line.trim());
        if !t.is_empty() && t[0].eq_ignore_ascii_case("FILE") {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            let kind = t.last().cloned().unwrap_or_else(|| "BINARY".into());
            out.push_str(&format!(
                "{}FILE \"{}\" {}\r\n",
                indent,
                english_name(k, disc.files.len()),
                kind
            ));
            k += 1;
        } else {
            out.push_str(line.trim_end_matches('\r'));
            out.push_str("\r\n");
        }
    }
    out
}

/// Writes the English disc into `out_dir`. Returns the path of the new .cue.
pub fn build(
    checked: &Checked,
    out_dir: &Path,
    allow_unknown: bool,
    progress: &mut dyn FnMut(u64, u64, &str),
) -> io::Result<PathBuf> {
    match &checked.state {
        Track1State::Original => {}
        Track1State::AlreadyPatched => {
            return err("This disc already has the English patch.");
        }
        Track1State::Unknown(_) if !allow_unknown => {
            return err(
                "This Track 01 doesn't match the known Japanese disc, so it wasn't patched.",
            );
        }
        Track1State::Unknown(_) => {}
    }
    let disc = &checked.disc;
    fs::create_dir_all(out_dir)?;

    let total: u64 = disc
        .files
        .iter()
        .map(|f| fs::metadata(&f.path).map(|m| m.len()).unwrap_or(0))
        .sum();
    let mut done: u64 = 0;

    for (i, f) in disc.files.iter().enumerate() {
        let dst = out_dir.join(english_name(i, disc.files.len()));
        if same_file(&f.path, &dst) {
            return err(
                "The output folder would overwrite your original files. Pick another folder.",
            );
        }
        let tmp = dst.with_extension("bin.part");
        let label = if i == 0 {
            "Patching Track 01".to_string()
        } else {
            format!("Copying track {} of {}", i + 1, disc.files.len())
        };
        progress(done, total, &label);
        {
            let mut out = io::BufWriter::with_capacity(1 << 20, File::create(&tmp)?);
            let mut src = BufReader::with_capacity(1 << 20, File::open(&f.path)?);
            if i == 0 {
                // Patched Track 01 (and, for single-file dumps, the rest of the disc after it)
                let mut t1 = vec![0u8; TRACK1_SIZE as usize];
                src.read_exact(&mut t1)?;
                apply_ppf(&mut t1, PATCH)?;
                if checked.state == Track1State::Original && sha1_hex(&t1) != PATCHED_SHA1 {
                    let _ = fs::remove_file(&tmp);
                    return err("Internal check failed: the patched data isn't what it should be. Nothing was written.");
                }
                out.write_all(&t1)?;
                done += TRACK1_SIZE;
                progress(done, total, &label);
            }
            let mut buf = vec![0u8; 1 << 20];
            loop {
                let n = src.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                done += n as u64;
                progress(done, total, &label);
            }
            out.flush()?;
        }
        if dst.exists() {
            fs::remove_file(&dst)?;
        }
        fs::rename(&tmp, &dst)?;
    }

    let cue_out = out_dir.join(format!("{}.cue", ENGLISH_BASE));
    fs::write(&cue_out, english_cue(disc))?;
    progress(total, total, "Done");
    Ok(cue_out)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Default output folder: "English Patch" next to the original .cue.
pub fn default_out_dir(disc: &Disc) -> PathBuf {
    disc.cue_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("English Patch")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_patch_is_ppf3() {
        assert_eq!(&PATCH[0..5], b"PPF30");
    }

    #[test]
    fn cue_tokens_keep_quoted_names() {
        let t = tokens(r#"FILE "Some Game (Track 01).bin" BINARY"#);
        assert_eq!(t, vec!["FILE", "Some Game (Track 01).bin", "BINARY"]);
    }

    #[test]
    fn english_names() {
        assert!(english_name(0, 19).ends_with("(English) (Track 01).bin"));
        assert!(english_name(18, 19).ends_with("(Track 19).bin"));
        assert!(english_name(0, 1).ends_with("(English).bin"));
    }
}
