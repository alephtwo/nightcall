use crate::ffmpeg::{CoverStatus, convert_file};
use anyhow::{Error, anyhow};
use clap::Parser;
use cli::Args;
use glob::glob;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};
use util::chunk;

mod cli;
mod ffmpeg;
mod util;

fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Verify ffmpeg is available before doing any work.
    ffmpeg::check_available()?;

    // Figure out which files we're working on.
    let files = list_files(&args.directory)?;

    // Break them out into chunks for each thread.
    let chunks = chunk(files, args.threads);

    // For each chunk, spawn a thread that invokes ffmpeg on each file.
    let mut threads: Vec<JoinHandle<Result<Vec<String>, Error>>> = Vec::with_capacity(chunks.len());
    let multi_progress = MultiProgress::new();
    let progress_style =
        ProgressStyle::with_template("[{eta:4}] {bar:.blue/gray} {pos}/{len} {msg}")?;

    for chunk in chunks {
        let progress = multi_progress.add(ProgressBar::new(chunk.len() as u64));
        progress.set_style(progress_style.clone());

        threads.push(thread::spawn(move || {
            let mut messages: Vec<String> = Vec::new();
            for file in &chunk {
                let path = file
                    .to_str()
                    .ok_or(anyhow!("path is not a string"))?
                    .to_string();
                progress.set_message(path.clone());
                match convert_file(file, args.purge)? {
                    CoverStatus::Embedded => {}
                    CoverStatus::FoundAndEmbedded(ref img) => messages.push(format!(
                        "info: embedded {} as cover art for {path}",
                        img.display()
                    )),
                    CoverStatus::NotFound => {
                        messages.push(format!("warning: no cover art found for {path}"))
                    }
                }
                progress.inc(1);
            }
            progress.finish();
            Ok(messages)
        }))
    }

    // Wait for all threads to finish and collect messages.
    let mut all_messages: Vec<String> = Vec::new();
    for thread in threads {
        all_messages.extend(thread.join().expect("couldn't join thread")?);
    }
    multi_progress.clear()?;

    for message in all_messages {
        println!("{message}");
    }

    Ok(())
}

fn list_files(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let pattern = directory.join("**").join("*.flac");
    let globstring = pattern
        .to_str()
        .ok_or(anyhow!("Non-UTF8 Paths are not allowed"))?;
    Ok(glob(globstring)?.filter_map(Result::ok).collect())
}
