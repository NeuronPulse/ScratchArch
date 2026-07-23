//! Minimal CLI example for the ScratchArch driver.
//!
//! Usage:
//!
//! ```bash
//! cargo run --example sairc -- tests/c_programs/hello.ll
//! cargo run --example sairc -- tests/c_programs/factorial.ll --opt=aggressive
//! ```

use std::env;
use std::process;

use scratcharch_driver::{CompileConfig, CompileDriver, OptLevel};

fn parse_args() -> (String, OptLevel) {
    let mut args = env::args().skip(1);
    let path = args
        .next()
        .unwrap_or_else(|| {
            eprintln!("usage: sairc <file.ll> [--opt=none|basic|aggressive]");
            process::exit(1);
        });

    let mut opt_level = OptLevel::Basic;
    for arg in args {
        if let Some(value) = arg.strip_prefix("--opt=") {
            opt_level = match value {
                "none" => OptLevel::None,
                "basic" => OptLevel::Basic,
                "aggressive" => OptLevel::Aggressive,
                _ => {
                    eprintln!("unknown opt level: {} (expected none|basic|aggressive)", value);
                    process::exit(1);
                }
            };
        }
    }

    (path, opt_level)
}

fn main() {
    let (path, opt_level) = parse_args();

    let config = CompileConfig {
        opt_level,
        ..CompileConfig::default()
    };
    let driver = CompileDriver::new(config);

    match driver.compile_and_run_file(&path) {
        Ok(Some(value)) => println!("{:?}", value),
        Ok(None) => println!("(void)"),
        Err(err) => {
            eprintln!("error: {}", err);
            process::exit(1);
        }
    }
}
