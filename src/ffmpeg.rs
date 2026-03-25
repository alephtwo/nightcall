use anyhow::{Error, anyhow};
use std::{fs, path::Path, process::Command};

pub fn convert_file(file: &Path, purge: bool) -> Result<(), Error> {
    let mut output = file.to_path_buf();
    output.set_extension("mp3");

    let out = Command::new("ffmpeg")
        // Input file
        .arg("-i")
        .arg(file)
        // Set output bitrate to 320 kbps
        .arg("-ab")
        .arg("320k")
        // Normalize to 44.1 kHz — Mazda infotainment rejects or mishandles
        // higher sample rates common in FLAC (e.g. 96 kHz)
        .arg("-ar")
        .arg("44100")
        // Force stereo output; FLAC may be mono or multichannel
        .arg("-ac")
        .arg("2")
        // Copy all metadata from the input stream
        .arg("-map_metadata")
        .arg("0")
        // Clear the TLEN (duration) tag so players derive duration from the
        // Xing/Info header instead, which reflects the actual encoded content
        .arg("-metadata")
        .arg("TLEN=")
        // Write ID3v2.3 tags (broadest player compatibility)
        .arg("-id3v2_version")
        .arg("3")
        // Encode audio with LAME MP3
        .arg("-codec:a")
        .arg("libmp3lame")
        // Output file
        .arg(output)
        .output()
        .expect("failed to execute");
    if !out.status.success() {
        return Err(anyhow!("{:?}", String::from_utf8(out.stderr)));
    }

    if purge {
        fs::remove_file(file).expect("failed to remove file");
    }

    Ok(())
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
        .output()
        .expect("failed to execute ffprobe");
    if !out.status.success() {
        return Err(anyhow!("{:?}", String::from_utf8(out.stderr)));
    }
    Ok(!out.stdout.trim_ascii().is_empty())
}
