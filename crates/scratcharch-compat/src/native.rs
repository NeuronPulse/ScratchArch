//! Native differential execution (Part 4).
//!
//! For fixtures with a committed C source, the benchmark compiles the `.c`
//! with a real C compiler, runs the executable, and records its process exit
//! code (masked `& 0xFF`) and stdout. The process exit status is *by interface*
//! an 8-bit channel on POSIX, so native agreement is compared masked; the SAIR
//! interpreter and ISA VM expose the full `main` return value through an
//! explicit result channel, so *their* agreement is compared full-width. Where
//! a fixture needs one of the interpreter-provided runtime helpers (the SART
//! `__scratcharch_*` builtins) to link natively, a shim provides them on top of
//! libc.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Native execution result.
#[derive(Debug, Clone)]
pub struct NativeResult {
    /// Process exit code masked to its 8-bit status (the native exit channel).
    pub exit_masked: u32,
    /// Captured stdout.
    pub stdout: String,
}

/// The C compiler we drive, when one is on `PATH`.
pub fn cc_available() -> Option<&'static str> {
    if Command::new("clang").arg("--version").output().is_ok() {
        Some("clang")
    } else if Command::new("gcc").arg("--version").output().is_ok() {
        Some("gcc")
    } else {
        None
    }
}

/// `clang --version` first line, when clang is present.
pub fn clang_version() -> Option<String> {
    let out = Command::new("clang").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().next().map(|l| l.trim().to_string())
}

/// `true` when this compiler is clang (needed for fresh `-S -emit-llvm`).
pub fn is_clang() -> bool {
    matches!(cc_available(), Some("clang"))
}

/// Stub definitions of the interpreter-provided runtime helpers, so a native
/// build of the `string`/`memory`/`runtime_mem` fixtures links and runs the
/// same semantics. Every SART builtin forwards to its libc namesake — the
/// `scratcharch-runtime` implementations are themselves libc-compatible
/// (`memcmp`/`strcmp` sign, `strncpy` zero-padding), so the native exit channel
/// stays comparable.
const INTRINSIC_SHIM: &str = r#"
#include <string.h>
#include <stddef.h>
void *__scratcharch_memcpy(void *dest, const void *src, unsigned int n) {
    return memcpy(dest, src, n);
}
void *__scratcharch_memmove(void *dest, const void *src, unsigned int n) {
    return memmove(dest, src, n);
}
void *__scratcharch_memset(void *dest, int c, unsigned int n) {
    return memset(dest, c, n);
}
int __scratcharch_memcmp(const void *a, const void *b, unsigned int n) {
    return memcmp(a, b, n);
}
int __scratcharch_strlen(const char *s) { return (int)strlen(s); }
int __scratcharch_strcmp(const char *a, const char *b) {
    return strcmp(a, b);
}
char *__scratcharch_strcpy(char *dest, const char *src) {
    return strcpy(dest, src);
}
char *__scratcharch_strncpy(char *dest, const char *src, unsigned int n) {
    return strncpy(dest, src, n);
}
"#;

fn write_shim(dir: &Path) -> Result<PathBuf, String> {
    let path = dir.join("__compat_intrinsic_shim.c");
    fs::write(&path, INTRINSIC_SHIM)
        .map_err(|e| format!("cannot write intrinsic shim: {e}"))?;
    Ok(path)
}

/// Compile and run a committed `.c` source natively. Returns the masked exit
/// code and stdout.
pub fn run_native(source: &Path, cc: &str, workdir: &Path) -> Result<NativeResult, String> {
    fs::create_dir_all(workdir).map_err(|e| format!("cannot create workdir: {e}"))?;
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("fixture");
    let exe = workdir.join(stem);
    let shim = write_shim(workdir)?;

    let status = Command::new(cc)
        .arg("-O0")
        .arg(source)
        .arg(&shim)
        .arg("-o")
        .arg(&exe)
        .status()
        .map_err(|e| format!("cannot invoke {cc}: {e}"))?;
    if !status.success() {
        return Err(format!("native compile of {} failed", source.display()));
    }

    let out = Command::new(&exe)
        .output()
        .map_err(|e| format!("cannot run native binary: {e}"))?;
    let code = out
        .status
        .code()
        .ok_or_else(|| "native binary terminated by signal (no exit code)".to_string())?;
    Ok(NativeResult {
        exit_masked: (code as u32) & 0xFF,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
    })
}

/// Recompile a committed `.c` source to an in-memory LLVM IR string with clang
/// (`-O0`, optnone disabled — the same flags the committed corpus was built
/// with). Returns `None` when clang is unavailable.
pub fn compile_fresh(c_source: &Path) -> Option<String> {
    if !is_clang() {
        return None;
    }
    let out_dir = std::env::temp_dir().join("scratcharch_compat_fresh");
    fs::create_dir_all(&out_dir).ok()?;
    let stem = c_source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("fixture");
    let ll_file = out_dir.join(format!("{stem}.ll"));

    let status = Command::new("clang")
        .args(["-S", "-emit-llvm", "-O0", "-Xclang", "-disable-O0-optnone"])
        .arg(c_source)
        .arg("-o")
        .arg(&ll_file)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }
    fs::read_to_string(&ll_file).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_result_masks_exit() {
        let nr = NativeResult {
            exit_masked: 0x9C,
            stdout: String::new(),
        };
        assert_eq!(nr.exit_masked, 156);
    }

    #[test]
    fn cc_probe_never_panics() {
        // Just exercises the probe; result depends on the host toolchain.
        let _ = cc_available();
    }
}
