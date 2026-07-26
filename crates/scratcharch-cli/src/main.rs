use clap::{Parser, Subcommand};

mod analyze_cmd;
mod build;
mod decompile;
mod diff;
mod graph;
mod inspect;

#[derive(Parser)]
#[command(name = "scratcharch", version, about = "ScratchArch developer toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile LLVM IR or ScratchGraph to a Scratch project.json
    Build {
        /// Input file (.ll or .sg)
        input: String,
        /// Output path for project.json (default: <input>.json)
        #[arg(short, long)]
        output: Option<String>,
        /// Optimization level: none, basic, aggressive
        #[arg(short, long, default_value = "basic")]
        optimize: String,
    },
    /// Analyze a Scratch project and produce a static analysis report
    Analyze {
        /// Input Scratch project (.json or .sb3)
        input: String,
        /// Output format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Decompile a Scratch project to SAIR
    Decompile {
        /// Input Scratch project (.json or .sb3)
        input: String,
        /// Output SAIR file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Generate DOT graph files for analysis
    Graph {
        /// Input Scratch project (.json or .sb3)
        input: String,
        /// Graph type: cfg, callgraph, project
        #[arg(short, long, default_value = "callgraph")]
        kind: String,
        /// Output DOT file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Display project structure summary
    Inspect {
        /// Input Scratch project (.json or .sb3)
        input: String,
    },
    /// Semantic diff between two Scratch projects
    Diff {
        /// First Scratch project
        a: String,
        /// Second Scratch project
        b: String,
        /// Output format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Build { input, output, optimize } => build::run(input, output, optimize),
        Command::Analyze { input, format, output } => analyze_cmd::run(input, format, output),
        Command::Decompile { input, output } => decompile::run(input, output),
        Command::Graph { input, kind, output } => graph::run(input, kind, output),
        Command::Inspect { input } => inspect::run(input),
        Command::Diff { a, b, format } => diff::run(a, b, format),
    };
    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
