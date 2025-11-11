use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, value_parser = path_exists)]
    pub directory: PathBuf,

    #[arg(short, long, default_value_t = num_cpus::get_physical().try_into().expect("failed to determine how many cpus"))]
    pub threads: usize,

    #[arg(
        short,
        long,
        default_value_t = false,
        help = "Remove original files after conversion"
    )]
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
