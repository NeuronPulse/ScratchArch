//! Tests for scratcharch-analyzer static analyses.

use scratcharch_analyzer::{
    semantic_diff, CallGraphAnalysis, CfgAnalysis, ComplexityMetrics, DotOutput,
    ReachabilityAnalysis, RecursionKind, Report, VariableUsageAnalyzer,
};
use scratcharch_scratchgraph::ir::{
    EventHat, Expr, Procedure, ProcedurePrototype, Project, Script, ScriptEntry, Sprite, Stage,
    Stmt, StopOption, Value as SgValue,
};

fn empty_project() -> Project {
    Project {
        stage: Stage {
            name: "Stage".to_string(),
            variables: Vec::new(),
            lists: Vec::new(),
            broadcasts: Vec::new(),
            scripts: Vec::new(),
            procedures: Vec::new(),
        },
        sprites: Vec::new(),
    }
}

fn make_proc(name: &str, body: Vec<Stmt>) -> Procedure {
    Procedure {
        prototype: ProcedurePrototype {
            name: name.to_string(),
            params: Vec::new(),
        },
        body,
        frame_size: 0,
    }
}

#[test]
fn test_cfg_simple_sequence() {
    let body = vec![
        Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::Literal(SgValue::Number(1.0)),
        },
        Stmt::SetVariable {
            var: "y".to_string(),
            value: Expr::Literal(SgValue::Number(2.0)),
        },
    ];
    let cfg = CfgAnalysis::new().build(&body);
    assert_eq!(cfg.nodes.len(), 4); // entry, exit, 2 statements
    assert_eq!(cfg.successors(cfg.entry_index()).len(), 1);
    assert_eq!(cfg.predecessors(cfg.exit_index()).len(), 1);
}

#[test]
fn test_cfg_if_branching() {
    let body = vec![Stmt::If {
        condition: Expr::Literal(SgValue::Bool(true)),
        then_body: vec![Stmt::Stop {
            option: StopOption::ThisScript,
        }],
        else_body: Vec::new(),
    }];
    let cfg = CfgAnalysis::new().build(&body);
    let successors = cfg.successors(2); // first statement node
    assert_eq!(successors.len(), 2);
}

#[test]
fn test_callgraph_direct_recursion() {
    let mut project = empty_project();
    let factorial = make_proc(
        "factorial",
        vec![Stmt::Call {
            proc: "factorial".to_string(),
            args: vec![],
        }],
    );
    project.stage.procedures.push(factorial);

    let graph = CallGraphAnalysis::new().analyze(&project);
    assert!(graph.is_recursive("factorial"));
    assert_eq!(graph.recursive.get("factorial"), Some(&RecursionKind::Direct));
}

#[test]
fn test_callgraph_mutual_recursion() {
    let mut project = empty_project();
    let a = make_proc(
        "a",
        vec![Stmt::Call {
            proc: "b".to_string(),
            args: vec![],
        }],
    );
    let b = make_proc(
        "b",
        vec![Stmt::Call {
            proc: "a".to_string(),
            args: vec![],
        }],
    );
    project.stage.procedures.push(a);
    project.stage.procedures.push(b);

    let graph = CallGraphAnalysis::new().analyze(&project);
    assert!(graph.is_recursive("a"));
    assert!(graph.is_recursive("b"));
    assert_eq!(graph.recursive.get("a"), Some(&RecursionKind::Mutual));
}

#[test]
fn test_variable_usage_dead_variable() {
    let mut project = empty_project();
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(
            EventHat::GreenFlag,
            vec![Stmt::SetVariable {
                var: "dead".to_string(),
                value: Expr::Literal(SgValue::Number(1.0)),
            }],
        ),
    });

    let usage = VariableUsageAnalyzer::new().analyze(&project);
    assert!(usage.dead_variables().contains(&"dead".to_string()));
}

#[test]
fn test_variable_usage_read_write() {
    let mut project = empty_project();
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(
            EventHat::GreenFlag,
            vec![
                Stmt::SetVariable {
                    var: "x".to_string(),
                    value: Expr::Literal(SgValue::Number(1.0)),
                },
                Stmt::SetVariable {
                    var: "x".to_string(),
                    value: Expr::Variable("x".to_string()),
                },
            ],
        ),
    });

    let usage = VariableUsageAnalyzer::new().analyze(&project);
    assert_eq!(usage.variables.get("x").unwrap().reads, 1);
    assert_eq!(usage.variables.get("x").unwrap().writes, 2);
}

#[test]
fn test_reachability_unreachable_broadcast() {
    let mut project = empty_project();
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, vec![]),
    });
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(
            EventHat::BroadcastReceived("missing".to_string()),
            vec![],
        ),
    });

    let unreachable = ReachabilityAnalysis::new().analyze(&project);
    assert_eq!(unreachable.len(), 1);
    assert!(matches!(&unreachable[0].hat, EventHat::BroadcastReceived(name) if name == "missing"));
}

#[test]
fn test_reachability_reachable_broadcast() {
    let mut project = empty_project();
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(
            EventHat::GreenFlag,
            vec![Stmt::Broadcast {
                message: Expr::Literal(SgValue::String("go".to_string())),
            }],
        ),
    });
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::BroadcastReceived("go".to_string()), vec![]),
    });

    let unreachable = ReachabilityAnalysis::new().analyze(&project);
    assert!(unreachable.is_empty());
}

#[test]
fn test_reachability_sprite_clicked_reachable_on_sprite() {
    let mut project = empty_project();
    let sprite = Sprite {
        name: "Sprite1".to_string(),
        variables: Vec::new(),
        lists: Vec::new(),
        scripts: vec![Script {
            entry: ScriptEntry::new(EventHat::SpriteClicked, vec![]),
        }],
        procedures: Vec::new(),
    };
    project.sprites.push(sprite);

    let unreachable = ReachabilityAnalysis::new().analyze(&project);
    assert!(unreachable.is_empty());
}

#[test]
fn test_dot_cfg_output() {
    let cfg = CfgAnalysis::new().build(&[]);
    let dot = cfg.to_dot("test");
    assert!(dot.starts_with("digraph test {"));
    assert!(dot.contains("Entry"));
    assert!(dot.contains("Exit"));
}

#[test]
fn test_dot_callgraph_output() {
    let mut project = empty_project();
    let a = make_proc(
        "a",
        vec![Stmt::Call {
            proc: "b".to_string(),
            args: vec![],
        }],
    );
    let b = make_proc(
        "b",
        vec![Stmt::Call {
            proc: "a".to_string(),
            args: vec![],
        }],
    );
    project.stage.procedures.push(a);
    project.stage.procedures.push(b);

    let callgraph = CallGraphAnalysis::new().analyze(&project);
    let dot = callgraph.to_dot("callgraph");
    assert!(dot.contains("digraph callgraph"));
    assert!(dot.contains("a") && dot.contains("b"));
}

#[test]
fn test_complexity_metrics() {
    let mut project = empty_project();
    let body = vec![
        Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::Literal(SgValue::Number(1.0)),
        },
        Stmt::If {
            condition: Expr::Literal(SgValue::Bool(true)),
            then_body: vec![
                Stmt::RepeatUntil {
                    condition: Expr::Literal(SgValue::Bool(false)),
                    body: vec![Stmt::Call {
                        proc: "foo".to_string(),
                        args: vec![],
                    }],
                },
            ],
            else_body: Vec::new(),
        },
    ];
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, body),
    });

    let metrics = ComplexityMetrics::new(&project);
    assert_eq!(metrics.script_count, 1);
    assert_eq!(metrics.total_statements, 2);
    assert_eq!(metrics.call_count, 1);
    assert!(metrics.max_nesting_depth >= 2);
}

#[test]
fn test_semantic_diff_identical() {
    let mut project = empty_project();
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, vec![Stmt::Stop {
            option: StopOption::ThisScript,
        }]),
    });

    let diffs = semantic_diff(&project, &project);
    assert!(diffs.is_empty());
}

#[test]
fn test_semantic_diff_different() {
    let mut a = empty_project();
    a.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, vec![]),
    });

    let mut b = empty_project();
    b.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, vec![]),
    });
    b.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::KeyPressed("space".to_string()), vec![]),
    });

    let diffs = semantic_diff(&a, &b);
    assert!(!diffs.is_empty());
}

#[test]
fn test_analysis_report_creation() {
    let mut project = empty_project();
    project.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, vec![Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::Literal(SgValue::Number(42.0)),
        }]),
    });

    let callgraph = CallGraphAnalysis::new().analyze(&project);
    let unreachable = ReachabilityAnalysis::new().analyze(&project);
    let usage = VariableUsageAnalyzer::new().analyze(&project);
    let report = Report::build(&project, &callgraph, &unreachable, &usage);

    let text = report.to_text();
    assert!(text.contains("Analysis Report"));
    assert!(text.contains("Scripts: 1"));

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("project_name"));
}

#[test]
fn test_analysis_cache_basic() {
    use scratcharch_analyzer::AnalysisCache;

    let mut cache = AnalysisCache::new();
    let project = empty_project();

    // Initially needs reanalysis.
    assert!(cache.needs_reanalysis(&project));

    let callgraph = CallGraphAnalysis::new().analyze(&project);
    let unreachable = ReachabilityAnalysis::new().analyze(&project);
    let usage = VariableUsageAnalyzer::new().analyze(&project);
    cache.store(&project, callgraph, unreachable, usage);

    // Same project should not need reanalysis.
    assert!(!cache.needs_reanalysis(&project));
}

#[test]
fn test_analysis_cache_changed_project() {
    use scratcharch_analyzer::AnalysisCache;

    let mut cache = AnalysisCache::new();
    let mut a = empty_project();
    let b = empty_project();

    a.stage.scripts.push(Script {
        entry: ScriptEntry::new(EventHat::GreenFlag, vec![]),
    });

    let callgraph = CallGraphAnalysis::new().analyze(&a);
    let unreachable = ReachabilityAnalysis::new().analyze(&a);
    let usage = VariableUsageAnalyzer::new().analyze(&a);
    cache.store(&a, callgraph, unreachable, usage);

    // Different project should trigger reanalysis.
    assert!(cache.needs_reanalysis(&b));
}

#[test]
fn test_analysis_cache_clear() {
    use scratcharch_analyzer::AnalysisCache;

    let mut cache = AnalysisCache::new();
    let project = empty_project();

    let callgraph = CallGraphAnalysis::new().analyze(&project);
    let unreachable = ReachabilityAnalysis::new().analyze(&project);
    let usage = VariableUsageAnalyzer::new().analyze(&project);
    cache.store(&project, callgraph, unreachable, usage);

    assert!(!cache.needs_reanalysis(&project));
    cache.clear();
    assert!(cache.needs_reanalysis(&project));
}

#[test]
fn test_cfg_dot_node_metadata() {
    let body = vec![
        Stmt::SetVariable {
            var: "x".to_string(),
            value: Expr::Literal(SgValue::Number(1.0)),
        },
    ];
    let cfg = CfgAnalysis::new().build(&body);
    let meta = cfg.node_metadata();
    // entry, exit, 1 statement
    assert_eq!(meta.len(), 3);
    assert!(meta[2].block_id_prefix.is_some());
    assert_eq!(meta[2].statement_count, 1);
}

#[test]
fn test_source_map_builder() {
    use scratcharch_scratchgraph::SourceMapBuilder;

    let json = serde_json::json!({
        "targets": [{
            "isStage": true,
            "name": "Stage",
            "variables": {},
            "lists": {},
            "broadcasts": {},
            "blocks": {}
        }]
    });

    let mut builder = SourceMapBuilder::new();
    let project = builder.parse(&json).expect("parse failed");
    assert_eq!(project.stage.name, "Stage");
    assert_eq!(builder.source_map.len(), 1);
    assert_eq!(builder.source_map[0].target_name, "Stage");
}

#[test]
fn test_source_location_construction() {
    use scratcharch_scratchgraph::SourceLocation;

    let loc = SourceLocation::new("Sprite1")
        .with_project_id("proj-123")
        .with_script_id("script-1")
        .with_block_id("block-abc")
        .with_opcode("event_whenflagclicked");

    assert_eq!(loc.sprite_name, "Sprite1");
    assert_eq!(loc.project_id, Some("proj-123".to_string()));
    assert_eq!(loc.block_id, Some("block-abc".to_string()));
}
