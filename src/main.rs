use clap::Parser;
use cli::Args;
use glob::glob;
use std::{
    fs,
    path::PathBuf,
    process::{Command, abort},
    thread::{self, JoinHandle},
};

mod cli;

fn main() {
    let args = Args::parse();

    // Figure out which files we're working on.
    let globstring = args.directory.join("**").join("*.flac");
    let files: Vec<PathBuf> = glob(globstring.to_str().expect("non-UTF8 path somehow"))
        .expect("failed to glob")
        .filter_map(Result::ok)
        .collect();

    // Break them out into chunks for each thread.
    let mut chunks: Vec<Vec<PathBuf>> = Vec::with_capacity(args.threads);
    for _ in 0..args.threads {
        chunks.push(Vec::new())
    }
    for (i, file) in files.iter().enumerate() {
        chunks
            .get_mut(i.rem_euclid(args.threads))
            .expect("unknown chunk")
            .push(file.to_path_buf());
    }

    // For each chunk, spawn a thread that invokes ffmpeg on each file.
    let mut threads: Vec<JoinHandle<()>> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        threads.push(thread::spawn(move || {
            for file in &chunk {
                println!("[{:?}] {}", thread::current().id(), file.to_str().unwrap());
                let mut output = file.clone();
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
                    eprintln!("it's broken: {:?}", out.stderr);
                    abort();
                }

                fs::remove_file(file).expect("failed to remove file")
            }
        }))
    }

    // Wait for all threads to finish.
    for thread in threads {
        thread.join().expect("couldn't join thread");
    }
}
