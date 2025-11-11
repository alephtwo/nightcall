use crate::ffmpeg::convert_file;
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

    // Figure out which files we're working on.
    let files = list_files(&args.directory)?;

    // Break them out into chunks for each thread.
    let chunks = chunk(files, args.threads);

    // For each chunk, spawn a thread that invokes ffmpeg on each file.
    let mut threads: Vec<JoinHandle<Result<(), Error>>> = Vec::with_capacity(chunks.len());
    let multi_progress = MultiProgress::new();
    let progress_style =
        ProgressStyle::with_template("[{eta:4}] {bar:.blue/gray} {pos}/{len} {msg}")?;

    for chunk in chunks {
        let progress = multi_progress.add(ProgressBar::new(chunk.len() as u64));
        progress.set_style(progress_style.clone());

        threads.push(thread::spawn(move || {
            for file in &chunk {
                let path = file
                    .to_str()
                    .ok_or(anyhow!("path is not a string"))?
                    .to_string();
                progress.set_message(path.to_string());
                convert_file(file, args.purge)?;
                progress.inc(1);
            }
            progress.finish();
            Ok(())
        }))
    }

    // Wait for all threads to finish.
    for thread in threads {
        thread.join().expect("couldn't join thread")?;
    }
    multi_progress.clear()?;
    Ok(())
}

fn list_files(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let pattern = directory.join("**").join("*.flac");
    let globstring = pattern
        .to_str()
        .ok_or(anyhow!("Non-UTF8 Paths are not allowed"))?;
    Ok(glob(globstring)?.filter_map(Result::ok).collect())
}
