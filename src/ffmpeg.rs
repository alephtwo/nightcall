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

pub fn convert_file(file: &Path, purge: bool) -> Result<CoverStatus, Error> {
    let mut output = file.to_path_buf();
    output.set_extension("mp3");

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

    // Use VBR V0 (highest quality variable bitrate)
    cmd.arg("-q:a").arg("0");
    // Normalize to 44.1 kHz — Mazda infotainment rejects or mishandles
    // higher sample rates common in FLAC (e.g. 96 kHz)
    cmd.arg("-ar").arg("44100");
    // Force stereo output; FLAC may be mono or multichannel
    cmd.arg("-ac").arg("2");
    // Copy all metadata from the input stream
    cmd.arg("-map_metadata").arg("0");
    // Clear the TLEN (duration) tag so players derive duration from the
    // Xing/Info header instead, which reflects the actual encoded content
    cmd.arg("-metadata").arg("TLEN=");
    // Write ID3v2.3 tags (broadest player compatibility)
    cmd.arg("-id3v2_version").arg("3");

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

    // Encode audio with LAME MP3
    cmd.arg("-codec:a").arg("libmp3lame");
    // Output file
    cmd.arg(&output);

    let out = cmd.output()?;
    if !out.status.success() {
        return Err(anyhow!("{}", String::from_utf8_lossy(&out.stderr)));
    }

    if purge {
        fs::remove_file(file)?;
    }

    Ok(status)
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
