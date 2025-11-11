use anyhow::{Error, anyhow};
use std::{fs, path::Path, process::Command};

pub fn convert_file(file: &Path) -> Result<(), Error> {
    let mut output = file.to_path_buf();
    output.set_extension("mp3");

    let out = Command::new("ffmpeg")
        .arg("-i")
        .arg(file)
        .arg("-ab")
        .arg("320k")
        .arg("-map_metadata")
        .arg("0")
        .arg("-id3v2_version")
        .arg("3")
        .arg("-codec:a")
        .arg("libmp3lame")
        .arg(output)
        .output()
        .expect("failed to execute");
    if !out.status.success() {
        return Err(anyhow!("{:?}", out.stderr));
    }

    fs::remove_file(file).expect("failed to remove file");
    Ok(())
}
