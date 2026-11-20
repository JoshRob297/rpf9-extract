mod bytes;
mod carve;
mod container;
mod extract;
mod ranges;
mod report;
mod scan;
mod zlib;

use memmap2::Mmap;
use report::Report;
use scan::Scan;
use std::{
    env,
    error::Error,
    fs::File,
    path::{Path, PathBuf},
    process::ExitCode,
};

const USAGE: &str = "\
usage: rpf9 [-n] [-v] <file> [outdir]

  -n, --list     report only, extract nothing
  -v, --verbose  add every archive's header fields to the report
  -h, --help     print this help

Extracts to out/<file stem> unless outdir is given. The directory must be empty.";

fn run() -> Result<(), Box<dyn Error>> {
    let (mut paths, mut list, mut verbose) = (Vec::new(), false, false);
    for arg in env::args_os().skip(1) {
        match arg.to_str() {
            Some("-n" | "--list") => list = true,
            Some("-v" | "--verbose") => verbose = true,
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return Ok(());
            }
            Some(option) if option.starts_with('-') => {
                return Err(format!("unknown option {option}\n{USAGE}").into());
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    let [file, rest @ ..] = paths.as_slice() else {
        return Err(USAGE.into());
    };
    let out = match rest {
        [] => Path::new("out").join(file.file_stem().unwrap_or_default()),
        [out] => out.clone(),
        _ => return Err(USAGE.into()),
    };

    // SAFETY: the map is only read, and nothing is expected to write to an
    // archive while it is being inspected.
    let map = |file| unsafe { Mmap::map(&file) };
    let data = File::open(file).and_then(map).map_err(|e| format!("{}: {e}", file.display()))?;
    let scan = Scan::new(&data);
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let report = Report { name: &name, scan: &scan };
    if verbose {
        print!("{report:#}");
    } else {
        print!("{report}");
    }
    if !list {
        extract::extract(&scan, &format!("{report:#}"), &out)?;
        let files = scan.files.len();
        let bytes: usize = scan.files.iter().map(|file| file.size()).sum();
        println!("\nwrote {files} files ({}) to {}", report::size(bytes), out.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("rpf9: {e}");
            ExitCode::FAILURE
        }
    }
}
