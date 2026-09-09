use clap::{Parser, Subcommand};

mod analyze_cmd;
mod build;
mod debug_cmd;
mod decompile;
mod diff;
mod graph;
mod inspect;
mod optimize_cmd;
mod pipeline_cmd;
mod sb3_cmd;
mod test_compat_cmd;
mod verify_cmd;

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
    /// Display project or SAIR module structure summary
    Inspect {
        /// Input file (.json, .sb3, or .sair)
        input: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run ScratchGraph optimization passes on a Scratch project.
    Optimize {
        /// Input project (.sb3 or .json)
        input: String,
        /// Output path (default: <input>_opt.sb3 or <input>_opt.json)
        #[arg(short, long)]
        output: Option<String>,
        /// Comma-separated pass names, or "all"
        #[arg(short, long, default_value = "all")]
        passes: String,
        /// Report format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Write report to file instead of stdout
        #[arg(long)]
        report: Option<String>,
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
    /// Verify a Scratch project: parse, graph validation, .sb3 roundtrip,
    /// semantic preservation, and per-pass transform preservation
    Verify {
        /// Input project (.sb3 or .json)
        input: String,
        /// Output the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Debug: trace SAIR <-> ScratchGraph <-> project.json mappings
    Debug {
        /// Input Scratch project (.json or .sb3)
        input: String,
        /// Optional SAIR file for cross-reference
        #[arg(short, long)]
        sair: Option<String>,
    },
    /// Run a full compilation pipeline with optional intermediate dumps
    Pipeline {
        /// Input file (.ll, .json, or .sb3)
        input: String,
        /// Output file for final result
        #[arg(short, long)]
        output: Option<String>,
        /// Dump intermediate files to the given directory
        #[arg(long)]
        dump: Option<String>,
        /// Pipeline mode: compile, roundtrip, decompile
        #[arg(short, long, default_value = "compile")]
        mode: String,
    },
    /// SB3 archive operations: unpack, build, inspect
    Sb3 {
        #[command(subcommand)]
        action: Sb3Action,
    },
    /// Run the LLVM compatibility benchmark over the committed corpus
    TestCompat {
        /// Restrict to fixtures carrying this feature tag (repeatable)
        #[arg(long, value_name = "FEATURE")]
        feature: Vec<String>,
        /// Restrict to fixtures whose recorded boundary stage is this
        /// (parser, sair, interpreter, vm, scratch; repeatable)
        #[arg(long, value_name = "STAGE")]
        stage: Vec<String>,
        /// Recompile every committed .c with clang before running
        #[arg(long)]
        fresh_clang: bool,
        /// Skip the native differential even when a C compiler is present
        #[arg(long)]
        no_native: bool,
        /// Report format: text or json
        #[arg(long, default_value = "text")]
        format: String,
        /// Compact text: headline, class counts, and any regressions
        #[arg(long, conflicts_with = "verbose")]
        quiet: bool,
        /// Verbose text: per-fixture detail
        #[arg(long, conflicts_with = "quiet")]
        verbose: bool,
        /// Force ASCII progress bars in text output
        #[arg(long)]
        ascii: bool,
        /// Repo root containing tests/corpus/llvm (default: auto-discovered)
        #[arg(long, value_name = "DIR")]
        corpus_root: Option<String>,
    },
}

#[derive(Subcommand)]
enum Sb3Action {
    /// Unpack an sb3 archive into project.json and assets/
    Unpack {
        /// Input .sb3 file
        input: String,
        /// Output directory (default: <input> without .sb3)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Rebuild an sb3 archive from its unpacked contents
    Build {
        /// Input .sb3 file
        input: String,
        /// Output .sb3 file (default: <input>_out.sb3)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Inspect an sb3 archive structure in detail
    Inspect {
        /// Input .sb3 file
        input: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Command::Build { input, output, optimize } => build::run(input, output, optimize),
        Command::Analyze { input, format, output } => analyze_cmd::run(input, format, output),
        Command::Decompile { input, output } => decompile::run(input, output),
        Command::Graph { input, kind, output } => graph::run(input, kind, output),
        Command::Inspect { input, json } => inspect::run(input, *json),
        Command::Optimize {
            input,
            output,
            passes,
            format,
            report,
        } => optimize_cmd::run(input, output, passes, format, report.as_deref()),
        Command::Diff { a, b, format } => diff::run(a, b, format),
        Command::Verify { input, json } => verify_cmd::run(input, *json),
        Command::Debug { input, sair } => debug_cmd::run(input, sair.as_deref()),
        Command::Pipeline { input, output, dump, mode } => {
            pipeline_cmd::run(input, output, dump, mode)
        }
        Command::Sb3 { action } => match action {
            Sb3Action::Unpack { input, output } => sb3_cmd::run_unpack(input, output.as_deref()),
            Sb3Action::Build { input, output } => sb3_cmd::run_build(input, output.as_deref()),
            Sb3Action::Inspect { input } => sb3_cmd::run_inspect(input),
        },
        Command::TestCompat {
            feature,
            stage,
            fresh_clang,
            no_native,
            format,
            quiet,
            verbose,
            ascii,
            corpus_root,
        } => test_compat_cmd::run(&test_compat_cmd::TestCompatArgs {
            features: feature.clone(),
            stages: stage.clone(),
            fresh_clang: *fresh_clang,
            no_native: *no_native,
            format: format.clone(),
            quiet: *quiet,
            verbose: *verbose,
            ascii: *ascii,
            corpus_root: corpus_root.clone(),
        }),
    };
    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
