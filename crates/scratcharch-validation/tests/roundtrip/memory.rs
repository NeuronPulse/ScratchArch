//! Memory: stack/heap-shaped programs.
//!
//! Two distinct cases live here.
//!
//! 1. **Native list-as-memory programs** are ordinary Scratch: a *declared*
//!    list stands in for memory and every access is a native `data_*`
//!    operation. These must round-trip exactly like any other Scratch code.
//!
//! 2. **ABI-lowered code** (`EnterFrame` / `FrameSet` / `FrameGet` /
//!    `HeapAlloc` / `HeapLoad` / `PopFrame`) is compiler output, not Scratch a
//!    human writes. The `JsonExporter` expands it into list idioms over
//!    `__scratcharch_stack` / `__scratcharch_heap`, and the parser does **not**
//!    reconstruct the ABI operations back (SCRATCH_SEMANTICS.md §5, §6). The
//!    framework therefore must *report* the difference rather than silently
//!    bless it. `assert_roundtrip_detects_loss` asserts exactly that the
//!    checker refuses to pass a lossy conversion.

use scratcharch_scratchgraph::ir::{
    EventHat, Expr, List, Project, Script, Stage, Stmt, Variable,
};

use crate::support::{assert_roundtrip_detects_loss, assert_roundtrip_preserves};

fn var(name: &str) -> Variable {
    Variable::new(name, name)
}

fn list(name: &str) -> List {
    List::new(name, name)
}

fn project_with(stage: Stage) -> Project {
    Project::new().with_stage(stage)
}

#[test]
fn native_list_as_a_stack_roundtrips() {
    // A declared list used explicitly as a stack: push, peek the top by
    // length-based index, pop. Every access is native Scratch.
    let mut stage = Stage::new("Stage");
    stage.add_list(list("memory"));
    stage.add_variable(var("top"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::DeleteAllOfList {
                list: "memory".into(),
            },
            // push 1
            Stmt::AddToList {
                list: "memory".into(),
                value: Expr::number(1.0),
            },
            // push 2
            Stmt::AddToList {
                list: "memory".into(),
                value: Expr::number(2.0),
            },
            // top := item at the end
            Stmt::SetVariable {
                var: "top".into(),
                value: Expr::list_item("memory", Expr::list_length("memory")),
            },
            // pop
            Stmt::DeleteListItem {
                list: "memory".into(),
                index: Expr::list_length("memory"),
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "native list-as-a-stack");
}

#[test]
fn native_list_as_memory_cells_roundtrips() {
    // A fixed-size "cell array" managed purely with native ops: write cell 2
    // via replace-item, read it via item-of with an arithmetic index.
    let mut stage = Stage::new("Stage");
    stage.add_list(list("memory"));
    stage.add_variable(var("cell"));
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::AddToList {
                list: "memory".into(),
                value: Expr::number(0.0),
            },
            Stmt::AddToList {
                list: "memory".into(),
                value: Expr::number(0.0),
            },
            Stmt::AddToList {
                list: "memory".into(),
                value: Expr::number(0.0),
            },
            Stmt::SetListItem {
                list: "memory".into(),
                index: Expr::number(2.0),
                value: Expr::number(42.0),
            },
            Stmt::SetVariable {
                var: "cell".into(),
                value: Expr::list_item(
                    "memory",
                    Expr::operator(
                        "operator_add",
                        vec![Expr::number(1.0), Expr::number(1.0)],
                    ),
                ),
            },
        ],
    ));
    assert_roundtrip_preserves(&project_with(stage), "native list-as-memory cells");
}

#[test]
fn abi_lowered_frame_code_is_detected_as_lossy() {
    // A compiled-style body using frame operations. The exporter expands it
    // into list idioms; the read-back is *not* the same IR, so the roundtrip
    // must be reported as semantically different.
    let mut stage = Stage::new("Stage");
    stage.add_script(Script::new(
        EventHat::GreenFlag,
        vec![
            Stmt::EnterFrame { slots: 3 },
            Stmt::FrameSet {
                offset: 0,
                value: Expr::number(7.0),
            },
            Stmt::SetVariable {
                var: "r".into(),
                value: Expr::FrameGet { offset: 0 },
            },
            Stmt::HeapAlloc {
                result_offset: 1,
                size: Expr::number(2.0),
            },
            Stmt::PopFrame { slots: 3 },
        ],
    ));
    assert_roundtrip_detects_loss(&project_with(stage), "ABI-lowered frame code");
}
