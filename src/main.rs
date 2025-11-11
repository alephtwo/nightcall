use crate::ffmpeg::convert_file;
use anyhow::{Error, anyhow};
use clap::Parser;
use cli::Args;
use glob::glob;
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
    for chunk in chunks {
        threads.push(thread::spawn(move || {
            for file in &chunk {
                println!("[{:?}] {}", thread::current().id(), file.to_str().unwrap());
                convert_file(file)?
            }
            Ok(())
        }))
    }

    // Wait for all threads to finish.
    for thread in threads {
        thread.join().expect("couldn't join thread")?;
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
