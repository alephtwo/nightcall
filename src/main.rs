use crate::ffmpeg::{CoverStatus, convert_file};
use anyhow::{Error, anyhow};
use clap::Parser;
use cli::Args;
use glob::glob;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
};

mod cli;
mod ffmpeg;

fn main() -> Result<(), Error> {
    let args = Args::parse();

    ffmpeg::check_available()?;

    let files = list_files(&args.directory)?;
    let messages = convert_files(files, args.threads, args.purge)?;
    for message in messages {
        println!("{message}");
    }

    Ok(())
}

fn convert_files(
    files: Vec<PathBuf>,
    num_threads: usize,
    purge: bool,
) -> Result<Vec<String>, Error> {
    let total = files.len() as u64;

    // Feed all files into a channel so threads can pull work on-demand.
    let (tx, rx) = mpsc::channel::<PathBuf>();
    for file in files {
        tx.send(file).expect("channel send failed");
    }
    drop(tx); // closing the sender signals end-of-work to receivers
    let rx = Arc::new(Mutex::new(rx));

    let multi_progress = MultiProgress::new();
    let overall_style =
        ProgressStyle::with_template("[{eta:4}] {bar:.blue/gray} {pos}/{len} files")?;
    let overall = multi_progress.add(ProgressBar::new(total));
    overall.set_style(overall_style);
    let thread_style = ProgressStyle::with_template("  {spinner:.blue} {msg}")?;

    let mut threads: Vec<JoinHandle<Result<Vec<String>, Error>>> = Vec::with_capacity(num_threads);
    for _ in 0..num_threads {
        let rx = Arc::clone(&rx);
        let overall = overall.clone();
        let thread_bar = multi_progress.add(ProgressBar::new_spinner());
        thread_bar.set_style(thread_style.clone());
        thread_bar.enable_steady_tick(std::time::Duration::from_millis(100));
        threads.push(thread::spawn(move || {
            worker(rx, overall, thread_bar, purge)
        }));
    }

    let mut all_messages: Vec<String> = Vec::new();
    for thread in threads {
        all_messages.extend(thread.join().expect("couldn't join thread")?);
    }
    overall.finish_and_clear();
    multi_progress.clear()?;

    Ok(all_messages)
}

fn worker(
    rx: Arc<Mutex<mpsc::Receiver<PathBuf>>>,
    overall: ProgressBar,
    thread_bar: ProgressBar,
    purge: bool,
) -> Result<Vec<String>, Error> {
    let mut messages: Vec<String> = Vec::new();
    loop {
        let file = {
            let lock = rx.lock().unwrap();
            lock.recv().ok()
        };
        match file {
            Some(file) => {
                thread_bar.set_message(file.display().to_string());
                match convert_file(&file, purge)? {
                    CoverStatus::Embedded => {}
                    CoverStatus::FoundAndEmbedded(ref img) => messages.push(format!(
                        "info: embedded {} as cover art for {}",
                        img.display(),
                        file.display()
                    )),
                    CoverStatus::NotFound => messages.push(format!(
                        "warning: no cover art found for {}",
                        file.display()
                    )),
                }
                overall.inc(1);
            }
            None => break,
        }
    }
    thread_bar.finish_and_clear();
    Ok(messages)
}

fn list_files(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let pattern = directory.join("**").join("*.flac");
    let globstring = pattern
        .to_str()
        .ok_or(anyhow!("Non-UTF8 Paths are not allowed"))?;
    Ok(glob(globstring)?.filter_map(Result::ok).collect())
}
