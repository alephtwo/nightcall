use anyhow::{Error, anyhow};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
};

pub enum CoverStatus {
    /// The source file already had embedded cover art.
    Embedded,
    /// No embedded art was found; a cover image was located and embedded.
    FoundAndEmbedded(PathBuf),
    /// No embedded art and no cover image file could be found.
    NotFound,
}

pub fn check_available() -> Result<(), Error> {
    let handles: Vec<_> = ["ffmpeg", "ffprobe"]
        .iter()
        .map(|&bin| {
            thread::spawn(move || {
                Command::new(bin).arg("-version").output().map_err(|_| {
                    anyhow!(
                        "{bin} is not available; please install it and ensure it is on the PATH"
                    )
                })
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked")?;
    }

    Ok(())
}

/// Average bitrate above which an MP3 is considered higher quality than V0.
/// V0 averages ~245 kbps (and rarely exceeds ~270 kbps over a whole track),
/// while the next step up in common use is 320 kbps CBR.
const V0_CEILING_BPS: u64 = 280_000;

/// Sample rate and channel count every output must have.
const TARGET_SAMPLE_RATE: u32 = 44_100;
const TARGET_CHANNELS: u32 = 2;

pub fn convert_file(file: &Path, purge: bool) -> Result<CoverStatus, Error> {
    let is_mp3 = file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"));
    let info = audio_info(file)?;
    let bitrate = audio_bitrate(file)?;
    // MP3 sources already at or below V0 and already 44.1 kHz stereo keep
    // their audio untouched (stream copy); only the cover art is reprocessed.
    let copy_audio = is_mp3
        && bitrate.is_some_and(|bps| bps <= V0_CEILING_BPS)
        && info.sample_rate == Some(TARGET_SAMPLE_RATE)
        && info.channels == Some(TARGET_CHANNELS);

    // ffmpeg cannot write over its own input, so MP3 sources are written to
    // a temporary file that replaces the original on success.
    let mut output = file.to_path_buf();
    if is_mp3 {
        let stem = file.file_stem().unwrap_or_default().to_string_lossy();
        output.set_file_name(format!("{stem}.nightcall-tmp.mp3"));
    } else {
        output.set_extension("mp3");
    }

    let embedded = has_cover_art(file)?;
    let external_cover = if embedded {
        None
    } else {
        find_cover_image(file)
    };

    let status = if embedded {
        CoverStatus::Embedded
    } else if let Some(ref img) = external_cover {
        CoverStatus::FoundAndEmbedded(img.clone())
    } else {
        CoverStatus::NotFound
    };

    let mut cmd = Command::new("ffmpeg");

    // Input file
    cmd.arg("-i").arg(file);

    // Optional cover image to embed when no art is embedded in the source
    if let Some(ref img) = external_cover {
        cmd.arg("-i").arg(img);
    }

    if copy_audio {
        // Already V0 or lower: don't touch the audio
        cmd.arg("-codec:a").arg("copy");
    } else {
        // Use VBR V0 (highest quality variable bitrate). Always used when
        // re-encoding, even for low-bitrate lossy sources, so the extra
        // lossy generation adds as little loss as possible.
        cmd.arg("-q:a").arg("0");
        // Normalize to 44.1 kHz — Mazda infotainment rejects or mishandles
        // higher sample rates common in FLAC (e.g. 96 kHz)
        cmd.arg("-ar").arg(TARGET_SAMPLE_RATE.to_string());
        // Force stereo output; sources may be mono or multichannel
        cmd.arg("-ac").arg(TARGET_CHANNELS.to_string());
        // Encode audio with LAME MP3
        cmd.arg("-codec:a").arg("libmp3lame");
    }
    // Copy all metadata from the input stream
    cmd.arg("-map_metadata").arg("0");
    // Clear the TLEN (duration) tag so players derive duration from the
    // Xing/Info header instead, which reflects the actual encoded content
    cmd.arg("-metadata").arg("TLEN=");
    // Clear the ISRC (TSRC) tag — Gracenote uses it as a lookup key to
    // identify tracks and override embedded metadata in car head units
    cmd.arg("-metadata").arg("TSRC=");
    // Write ID3v2.3 tags (broadest player compatibility)
    cmd.arg("-id3v2_version").arg("3");
    // Also write ID3v1 tags alongside ID3v2.3; some car head units prefer
    // ID3v1 and will fall back to Gracenote DB lookups if it is absent
    cmd.arg("-write_id3v1").arg("1");

    if external_cover.is_some() {
        // Map audio from first input and image from second input
        cmd.arg("-map").arg("0:a");
        cmd.arg("-map").arg("1:v");
    } else {
        // Map audio and any embedded cover art from the same input
        cmd.arg("-map").arg("0:a");
        cmd.arg("-map").arg("0:v?");
    }

    // Re-encode cover art as JPEG (broadest car stereo compatibility),
    // scaling down to at most 500x500 while preserving aspect ratio
    cmd.arg("-codec:v").arg("mjpeg").arg("-q:v").arg("2");
    cmd.arg("-vf")
        .arg("scale='min(500,iw)':'min(500,ih)':force_original_aspect_ratio=decrease");
    // Mark the video stream as an attached picture (cover art)
    cmd.arg("-disposition:v").arg("attached_pic");

    // Output file
    cmd.arg(&output);

    let out = cmd.output()?;
    if !out.status.success() {
        if is_mp3 {
            let _ = fs::remove_file(&output);
        }
        return Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr)));
    }

    if is_mp3 {
        // The original is replaced in place, so `purge` has nothing left to remove
        fs::rename(&output, file)?;
    } else if purge {
        fs::remove_file(file)?;
    }

    Ok(status)
}

struct AudioInfo {
    sample_rate: Option<u32>,
    channels: Option<u32>,
}

fn audio_info(file: &Path) -> Result<AudioInfo, Error> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "a:0"])
        .args(["-show_entries", "stream=sample_rate,channels"])
        .args(["-of", "default=nw=1"])
        .arg(file)
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr)));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .map(str::trim)
    };
    Ok(AudioInfo {
        sample_rate: field("sample_rate").and_then(|v| v.parse().ok()),
        channels: field("channels").and_then(|v| v.parse().ok()),
    })
}

/// Average audio bitrate in bits per second, or `None` if it can't be determined.
fn audio_bitrate(file: &Path) -> Result<Option<u64>, Error> {
    let probe = |entries: &str| -> Result<Option<u64>, Error> {
        let mut cmd = Command::new("ffprobe");
        cmd.args(["-v", "error"]);
        if entries.starts_with("stream") {
            cmd.args(["-select_streams", "a:0"]);
        }
        let out = cmd
            .args(["-show_entries", entries, "-of", "csv=p=0"])
            .arg(file)
            .output()?;
        if !out.status.success() {
            return Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr)));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().parse().ok())
    };
    // Some VBR streams report no stream bitrate; fall back to the container's
    // (which includes cover art, so it can only overestimate)
    match probe("stream=bit_rate")? {
        Some(bps) => Ok(Some(bps)),
        None => probe("format=bit_rate"),
    }
}

pub fn has_cover_art(file: &Path) -> Result<bool, Error> {
    let out = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        // Select video streams only (cover art is stored as a video stream in audio files)
        .arg("-select_streams")
        .arg("v")
        .arg("-show_entries")
        .arg("stream=codec_name")
        .arg("-of")
        .arg("csv=p=0")
        .arg(file)
        .output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(!out.stdout.trim_ascii().is_empty())
}

pub fn find_cover_image(file: &Path) -> Option<PathBuf> {
    let dir = file.parent()?;
    let candidates = [
        "cover.jpg",
        "cover.jpeg",
        "cover.png",
        "folder.jpg",
        "folder.jpeg",
        "folder.png",
        "artwork.jpg",
        "artwork.jpeg",
        "artwork.png",
        "album.jpg",
        "album.jpeg",
        "album.png",
        "front.jpg",
        "front.jpeg",
        "front.png",
    ];
    for name in candidates {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}
