//! Real-clang corpus: per-fixture 5-surface record + native-reference
//! differential.
//!
//! ## Part 8 — the five-surface record
//!
//! For every committed `tests/c_programs/<name>.{c,ll}` fixture this harness
//! walks the whole toolchain and pins one verdict per surface:
//!
//! 1. Parser — `translate_llvm` accepts the committed `.ll`.
//! 2. SAIR — the translated module passes `IrModule::validate`.
//! 3. Interpreter — the SAIR interpreter returns the expected exit value.
//! 4. VM — the ISA VM returns the same value *or* rejects the module with an
//!    explicit diagnostic that names the missing ISA/runtime capability (never
//!    a silent wrong result).
//! 5. Scratch — `ScratchGraphLowerer::lower` either constructs a project or
//!    fails with a diagnostic. Construction is a *record only*: §6.2 of
//!    `LLVM_COMPATIBILITY.md` declares the LLVM→Scratch path out of the v0.2
//!    scope, so an `Ok` project must not be read as a semantic claim — it only
//!    means the lowerer did not reject the module.
//!
//! This test runs on the committed fixtures and needs no C compiler.
//!
//! ## Part 9 — the native-reference differential
//!
//! The second test recompiles each `.c` with a real C compiler, runs the native
//! executable, and cross-checks all three execution surfaces live: the native
//! process exit code, the SAIR interpreter result, and the ISA VM result.
//! Where the VM is supported all three must agree; where it is not, the VM must
//! reject the module with the pinned diagnostic (never a silent wrong value).
//! Exit codes are compared `& 0xFF` because `main`'s return is the 8-bit
//! process exit status on POSIX. The test skips when no C compiler is on
//! `PATH`, mirroring `corpus_clang_tests.rs`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use scratcharch_driver::config::{CompileConfig, ExecutionBackend, OptLevel};
use scratcharch_driver::driver::{CompileDriver, ExecutionValue};
use scratcharch_llvm::translate_llvm;
use scratcharch_scratchgraph::ScratchGraphLowerer;

fn project_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // scratcharch-pipeline is at crates/scratcharch-pipeline/
    p.pop();
    p.pop();
    p
}

fn c_programs_dir() -> PathBuf {
    project_root().join("tests").join("c_programs")
}

/// Expected VM behavior for a committed fixture.
enum VmVerdict {
    /// The VM executes the module and returns this exact value.
    Exact(u32),
    /// The VM rejects the module; `err` is a substring of the diagnostic.
    Rejected(&'static str),
}

/// Expected Scratch-lower behavior for a committed fixture (construction only).
enum ScratchVerdict {
    /// The lowerer constructs a project (no semantic claim; see module docs).
    Constructs,
    /// The lowerer rejects the module; `err` is a substring of the diagnostic.
    Rejected(&'static str),
}

/// One committed real-clang fixture and its expected verdict on each surface.
struct Fixture {
    name: &'static str,
    expected: u32,
    vm: VmVerdict,
    scratch: ScratchVerdict,
}

fn fixtures() -> Vec<Fixture> {
    use ScratchVerdict::{Constructs, Rejected as ScratchRejected};
    use VmVerdict::{Exact, Rejected as VmRejected};
    vec![
        Fixture { name: "hello", expected: 42, vm: Exact(42), scratch: Constructs },
        Fixture { name: "add", expected: 42, vm: Exact(42), scratch: Constructs },
        Fixture { name: "factorial", expected: 120, vm: Exact(120), scratch: Constructs },
        Fixture { name: "fib", expected: 55, vm: Exact(55), scratch: Constructs },
        Fixture { name: "array", expected: 42, vm: Exact(42), scratch: Constructs },
        Fixture { name: "struct", expected: 30, vm: Exact(30), scratch: Constructs },
        Fixture { name: "pointer", expected: 42, vm: Exact(42), scratch: Constructs },
        Fixture {
            name: "string",
            expected: 5,
            vm: VmRejected("undefined function: __scratcharch_strlen"),
            scratch: Constructs,
        },
        Fixture {
            name: "memory",
            expected: 6,
            // Constant-length `__scratcharch_memcpy` is expanded by the
            // translator into byte loads/stores, so it now runs exact on the VM.
            vm: Exact(6),
            scratch: Constructs,
        },
        Fixture { name: "recursion", expected: 15, vm: Exact(15), scratch: Constructs },
        Fixture {
            name: "intrinsics",
            expected: 2_018_928_754,
            vm: VmRejected("undefined function: llvm.bswap.i16"),
            scratch: Constructs,
        },
        Fixture { name: "signed", expected: 78, vm: Exact(78), scratch: Constructs },
        Fixture {
            name: "memintrin",
            expected: 1,
            // Constant-length `llvm.memcpy`/`llvm.memmove`/`llvm.memset` are
            // expanded by the translator, so the VM runs the fixture exactly
            // (the overlapping `memmove` stays well-defined via the load-all
            // expansion).
            vm: Exact(1),
            scratch: Constructs,
        },
        Fixture {
            name: "globals",
            expected: 95,
            // Sub-word/byte leaves (strings, i8/i16) are byte-exact on the VM:
            // single-limb Load/Store are width-accurate, so the byte image runs
            // on both backends and agrees with native (exit 95). The byte-exact
            // ScratchGraph heap constructs the same project (SCRATCH_MEMORY.md).
            vm: Exact(95),
            scratch: Constructs,
        },
        Fixture {
            name: "signedcmp",
            expected: 59,
            vm: Exact(59),
            scratch: Constructs,
        },
        Fixture {
            name: "i64arith",
            expected: 8,
            vm: Exact(8),
            scratch: Constructs,
        },
        Fixture {
            name: "bytes",
            // Byte/sub-word globals and mixed-width loads/stores: the VM lowers
            // i8/i16 to byte ops and reads byte-exact static data, so it agrees
            // with native (and the interpreter) on the full checksum (exit 412,
            // masked to 156 by the 8-bit process status). On Scratch the byte
            // model constructs; the fixture still rejects at `and` (bitwise has
            // no Scratch operator).
            expected: 412,
            vm: Exact(412),
            scratch: ScratchRejected("and cannot be lowered to Scratch numbers"),
        },
        Fixture {
            name: "reinterp",
            // ptrtoint/inttoptr of a 32-bit SA48 pointer is a zero-cost cell
            // copy on the VM, so pointer/integer reinterpretation round-trips
            // exactly on both engines; the Scratch backend builds the same
            // project (byte-exact heap).
            expected: 331,
            vm: Exact(331),
            scratch: Constructs,
        },
    ]
}

/// Run a committed `.ll` through one driver backend.
fn run_backend(path: &PathBuf, backend: ExecutionBackend) -> Result<Option<ExecutionValue>, String> {
    let config = CompileConfig {
        opt_level: OptLevel::Basic,
        backend,
        ..CompileConfig::default()
    };
    CompileDriver::new(config)
        .compile_and_run_file(path)
        .map_err(|e| e.to_string())
}

/// Part 8 — pin the parser / SAIR / interpreter / VM / Scratch verdicts for
/// every committed real-clang fixture. Runs from the committed `.ll` files, no
/// C compiler required.
#[test]
fn five_surface_record() {
    for fx in fixtures() {
        let ll = c_programs_dir().join(format!("{}.ll", fx.name));
        let text = fs::read_to_string(&ll).unwrap_or_else(|e| {
            panic!("{}: cannot read committed fixture: {e}", fx.name)
        });

        // Surfaces 1 + 2: parser and SAIR validation.
        let module = match translate_llvm(&text) {
            Ok(m) => m,
            Err(e) => panic!("{}: parser FAILED: {e}", fx.name),
        };
        if let Err(e) = module.validate() {
            panic!("{}: SAIR validation FAILED: {e}", fx.name);
        }

        // Surface 3: interpreter returns the exact exit value.
        let interp = run_backend(&ll, ExecutionBackend::Interpreter)
            .unwrap_or_else(|e| panic!("{}: interpreter errored: {e}", fx.name));
        match interp {
            Some(ExecutionValue::I32(v)) => {
                assert_eq!(v, fx.expected, "{}: interpreter exit", fx.name)
            }
            other => panic!("{}: interpreter expected I32({}), got {other:?}", fx.name, fx.expected),
        }

        // Surface 4: VM either returns the exact value or rejects with an
        // explicit diagnostic — never a silent wrong result.
        let vm = run_backend(&ll, ExecutionBackend::Vm);
        match &vm {
            Ok(Some(ExecutionValue::I32(v))) => match &fx.vm {
                VmVerdict::Exact(expected) => assert_eq!(v, expected, "{}: VM exit", fx.name),
                VmVerdict::Rejected(diag) => {
                    panic!(
                        "{}: VM expected rejection containing {:?}, but returned I32({v})",
                        fx.name, diag
                    );
                }
            },
            Ok(other) => match &fx.vm {
                VmVerdict::Exact(expected) => {
                    panic!("{}: VM expected I32({expected}), got {other:?}", fx.name);
                }
                VmVerdict::Rejected(diag) => {
                    panic!(
                        "{}: VM expected rejection containing {:?}, but returned {other:?}",
                        fx.name, diag
                    );
                }
            },
            Err(e) => match &fx.vm {
                VmVerdict::Exact(expected) => {
                    panic!("{}: VM expected I32({expected}), got Err: {e}", fx.name);
                }
                VmVerdict::Rejected(diag) => {
                    assert!(
                        e.contains(diag),
                        "{}: VM diagnostic should contain {:?}, got: {e}",
                        fx.name,
                        diag
                    );
                }
            },
        }

        // Surface 5: Scratch lower either constructs (record only) or rejects
        // with a diagnostic that names the unlowerable construct.
        match ScratchGraphLowerer::new().lower(&module) {
            Ok(_project) => match &fx.scratch {
                ScratchVerdict::Constructs => {}
                ScratchVerdict::Rejected(diag) => {
                    panic!(
                        "{}: Scratch expected rejection containing {:?}, but constructed a project",
                        fx.name, diag
                    );
                }
            },
            Err(e) => match &fx.scratch {
                ScratchVerdict::Constructs => {
                    panic!("{}: Scratch expected to construct, got Err: {e}", fx.name);
                }
                ScratchVerdict::Rejected(diag) => {
                    let msg = e.to_string();
                    assert!(
                        msg.contains(diag),
                        "{}: Scratch diagnostic should contain {:?}, got: {msg}",
                        fx.name,
                        diag
                    );
                }
            },
        }
    }
}

/// Stub definitions of the runtime intrinsics the `string`/`memory` fixtures
/// call, so a native C build of those fixtures links against libc and executes
/// the same semantics the SAIR interpreter provides.
const INTRINSIC_SHIM: &str = r#"
#include <string.h>
#include <stddef.h>
void *__scratcharch_memcpy(void *dest, const void *src, unsigned int n) {
    return memcpy(dest, src, n);
}
int __scratcharch_strlen(const char *s) { return (int)strlen(s); }
"#;

fn cc_available() -> Option<&'static str> {
    if Command::new("clang").arg("--version").output().is_ok() {
        Some("clang")
    } else if Command::new("gcc").arg("--version").output().is_ok() {
        Some("gcc")
    } else {
        None
    }
}

/// Part 9 — three-way execution differential: native reference vs SAIR
/// interpreter vs ISA VM.
///
/// For every committed `.c` fixture:
///
/// * compile the `.c` with a real C compiler, run the native executable, and
///   record its process exit code;
/// * run the committed `.ll` on the SAIR interpreter and on the ISA VM;
/// * require the interpreter to agree with the native exit code;
/// * require the VM to agree with the native exit code where the VM is
///   supported, and to *reject* the module with the pinned diagnostic where it
///   is not — never a silent fallback to a wrong value.
///
/// Exit codes are compared `& 0xFF` because `main`'s return value is the 8-bit
/// process exit status on POSIX. Skips when no C compiler is on `PATH`,
/// mirroring `corpus_clang_tests.rs`.
#[test]
fn native_reference_differential() {
    let Some(cc) = cc_available() else {
        eprintln!("SKIP native_reference_differential (no C compiler found)");
        return;
    };

    let out_dir = std::env::temp_dir().join("scratcharch_native_corpus");
    fs::create_dir_all(&out_dir).expect("create native corpus temp dir");
    let shim = out_dir.join("intrinsic_shim.c");
    fs::write(&shim, INTRINSIC_SHIM).expect("write intrinsic shim");

    for fx in fixtures() {
        let c_src = c_programs_dir().join(format!("{}.c", fx.name));
        let ll = c_programs_dir().join(format!("{}.ll", fx.name));
        let exe = out_dir.join(fx.name);

        let status = Command::new(cc)
            .arg("-O0")
            .arg(&c_src)
            .arg(&shim)
            .arg("-o")
            .arg(&exe)
            .status()
            .unwrap_or_else(|e| panic!("{}: cannot invoke {cc}: {e}", fx.name));
        assert!(
            status.success(),
            "{}: native compile of {}.c failed",
            fx.name,
            fx.name
        );

        let run = Command::new(&exe)
            .status()
            .unwrap_or_else(|e| panic!("{}: cannot run native binary: {e}", fx.name));
        let native_exit = (run
            .code()
            .unwrap_or_else(|| {
                panic!("{}: native binary terminated by signal (no exit code)", fx.name)
            }) as u32)
            & 0xFF;

        // SAIR interpreter must agree with native execution.
        let interp = run_backend(&ll, ExecutionBackend::Interpreter)
            .unwrap_or_else(|e| panic!("{}: interpreter errored: {e}", fx.name));
        match interp {
            Some(ExecutionValue::I32(v)) => {
                assert_eq!(
                    v & 0xFF,
                    native_exit,
                    "{}: interpreter exit diverges from native execution",
                    fx.name
                );
            }
            other => panic!("{}: interpreter expected I32, got {other:?}", fx.name),
        }

        // ISA VM must agree with native where supported, and reject with the
        // pinned diagnostic where not — never a silent wrong value.
        let vm = run_backend(&ll, ExecutionBackend::Vm);
        match &vm {
            Ok(Some(ExecutionValue::I32(v))) => match &fx.vm {
                VmVerdict::Exact(_) => {
                    assert_eq!(
                        v & 0xFF,
                        native_exit,
                        "{}: VM exit diverges from native execution",
                        fx.name
                    );
                }
                VmVerdict::Rejected(diag) => {
                    panic!(
                        "{}: VM expected rejection containing {:?}, but ran (I32({v}))",
                        fx.name, diag
                    );
                }
            },
            Ok(other) => panic!("{}: VM expected I32, got {other:?}", fx.name),
            Err(e) => match &fx.vm {
                VmVerdict::Rejected(diag) => {
                    assert!(
                        e.contains(diag),
                        "{}: VM diagnostic should contain {:?}, got: {e}",
                        fx.name,
                        diag
                    );
                }
                VmVerdict::Exact(_) => {
                    panic!("{}: VM expected to run exactly, got Err: {e}", fx.name);
                }
            },
        }
    }
}
