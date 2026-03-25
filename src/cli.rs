use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, value_parser = path_exists)]
    pub directory: PathBuf,

    #[arg(short, long, default_value_t = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1), value_parser = parse_threads)]
    pub threads: usize,

    #[arg(short, long, help = "Remove original files after conversion")]
    pub purge: bool,
}

fn path_exists(s: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(s);
    if p.exists() && p.is_dir() {
        Ok(p)
    } else {
        Err("Path must exist and be a directory".to_string())
    }
}

fn parse_threads(s: &str) -> Result<usize, String> {
    let n: usize = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", s))?;
    if n >= 1 {
        Ok(n)
    } else {
        Err("number of threads must be at least 1".to_string())
    }
}
