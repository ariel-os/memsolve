use std::{error::Error, fs::File};

use clap::Parser;
use memsolve::{Memory, layout::Layout};

#[derive(Parser)]
#[command(version, about, long_about = "Multi-file yaml to memory.x parser")]
struct Cli {
    output: String,
    base: String,
    sections: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    let bf = File::open(args.base)?;
    println!("reading base layout");
    let mut base: Memory<_> = yaml_serde::from_reader(bf)?;

    println!("Reading extra files");
    for extra in args.sections {
        let f = File::open(extra)?;
        let sections: Layout<_> = yaml_serde::from_reader(f)?;
        base.layout_mut().merge(&sections);
    }

    let resolved = base.resolve_layout()?;

    let memory_layout = resolved.into_memory();

    memory_layout.to_file(args.output)?;
    Ok(())
}
