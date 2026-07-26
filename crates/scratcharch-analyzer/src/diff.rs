use scratcharch_scratchgraph::ir::{Project, Sprite, Stage};

pub enum DiffFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DiffEntry {
    MismatchedSprites { a: usize, b: usize },
    MismatchedScripts { target: String, a: usize, b: usize },
    MismatchedProcedures { target: String, a: usize, b: usize },
    MismatchedVariables { target: String, a: usize, b: usize },
    MismatchedLists { target: String, a: usize, b: usize },
    MismatchedBroadcasts { a: usize, b: usize },
    ProcedureBodyDiff { target: String, name: String, detail: String },
    ScriptBodyDiff { target: String, index: usize, detail: String },
    VariableScopeDiff { target: String, name: String, a_scope: String, b_scope: String },
    ListScopeDiff { target: String, name: String, a_scope: String, b_scope: String },
    ProcedureParamDiff { target: String, name: String, a_params: usize, b_params: usize },
    ProcedureBodySize { target: String, name: String, a: usize, b: usize },
    ScriptBodySize { target: String, index: usize, a: usize, b: usize },
}

#[derive(Debug, Clone, Default)]
pub struct DiffResult {
    pub entries: Vec<DiffEntry>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn format(&self, fmt: DiffFormat) -> String {
        match fmt {
            DiffFormat::Text => self.format_text(),
            DiffFormat::Json => self.format_json(),
        }
    }

    fn format_text(&self) -> String {
        if self.entries.is_empty() {
            return "No semantic differences found.".to_string();
        }
        let mut out = String::new();
        out.push_str(&format!("Semantic differences: {}\n", self.entries.len()));
        for (i, entry) in self.entries.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, entry.summary()));
        }
        out
    }

    fn format_json(&self) -> String {
        let items: Vec<String> = self
            .entries
            .iter()
            .map(|e| format!("{{ \"diff\": \"{}\" }}", e.summary().replace('"', "'")))
            .collect();
        format!("{{ \"differences\": [{}], \"count\": {} }}", items.join(",\n"), self.entries.len())
    }
}

impl DiffEntry {
    fn summary(&self) -> String {
        match self {
            DiffEntry::MismatchedSprites { a, b } => format!("sprite count differs: {} vs {}", a, b),
            DiffEntry::MismatchedScripts { target, a, b } => {
                format!("{} script count differs: {} vs {}", target, a, b)
            }
            DiffEntry::MismatchedProcedures { target, a, b } => {
                format!("{} procedure count differs: {} vs {}", target, a, b)
            }
            DiffEntry::MismatchedVariables { target, a, b } => {
                format!("{} variable count differs: {} vs {}", target, a, b)
            }
            DiffEntry::MismatchedLists { target, a, b } => {
                format!("{} list count differs: {} vs {}", target, a, b)
            }
            DiffEntry::MismatchedBroadcasts { a, b } => format!("broadcast count differs: {} vs {}", a, b),
            DiffEntry::ProcedureBodyDiff { target, name, detail } => {
                format!("{} procedure '{}' body differs: {}", target, name, detail)
            }
            DiffEntry::ScriptBodyDiff { target, index, detail } => {
                format!("{} script {} body differs: {}", target, index, detail)
            }
            DiffEntry::VariableScopeDiff { target, name, a_scope, b_scope } => {
                format!("{} variable '{}' scope: {} vs {}", target, name, a_scope, b_scope)
            }
            DiffEntry::ListScopeDiff { target, name, a_scope, b_scope } => {
                format!("{} list '{}' scope: {} vs {}", target, name, a_scope, b_scope)
            }
            DiffEntry::ProcedureParamDiff { target, name, a_params, b_params } => {
                format!("{} procedure '{}' params: {} vs {}", target, name, a_params, b_params)
            }
            DiffEntry::ProcedureBodySize { target, name, a, b } => {
                format!("{} procedure '{}' body size: {} vs {}", target, name, a, b)
            }
            DiffEntry::ScriptBodySize { target, index, a, b } => {
                format!("{} script {} body size: {} vs {}", target, index, a, b)
            }
        }
    }
}

pub fn semantic_diff(a: &Project, b: &Project) -> DiffResult {
    let mut result = DiffResult::default();
    diff_stage(&a.stage, &b.stage, &mut result);
    diff_sprites(&a.sprites, &b.sprites, &mut result);
    result
}

fn diff_stage(a: &Stage, b: &Stage, result: &mut DiffResult) {
    diff_target_contents("Stage", a.variables.len(), b.variables.len(), "variables", result);
    diff_target_contents("Stage", a.lists.len(), b.lists.len(), "lists", result);
    diff_target_contents("Stage", a.broadcasts.len(), b.broadcasts.len(), "broadcasts", result);
    diff_target_contents("Stage", a.scripts.len(), b.scripts.len(), "scripts", result);
    diff_target_contents("Stage", a.procedures.len(), b.procedures.len(), "procedures", result);

    let min_scripts = a.scripts.len().min(b.scripts.len());
    for i in 0..min_scripts {
        let sa_len = a.scripts[i].entry.body.len();
        let sb_len = b.scripts[i].entry.body.len();
        if sa_len != sb_len {
            result.entries.push(DiffEntry::ScriptBodySize {
                target: "Stage".to_string(),
                index: i,
                a: sa_len,
                b: sb_len,
            });
        }
    }

    let min_procs = a.procedures.len().min(b.procedures.len());
    for i in 0..min_procs {
        let pa = &a.procedures[i];
        let pb = &b.procedures[i];
        if pa.prototype.params.len() != pb.prototype.params.len() {
            result.entries.push(DiffEntry::ProcedureParamDiff {
                target: "Stage".to_string(),
                name: pa.prototype.name.clone(),
                a_params: pa.prototype.params.len(),
                b_params: pb.prototype.params.len(),
            });
        }
        if pa.body.len() != pb.body.len() {
            result.entries.push(DiffEntry::ProcedureBodySize {
                target: "Stage".to_string(),
                name: pa.prototype.name.clone(),
                a: pa.body.len(),
                b: pb.body.len(),
            });
        }
    }
}

fn diff_sprites(a: &[Sprite], b: &[Sprite], result: &mut DiffResult) {
    if a.len() != b.len() {
        result.entries.push(DiffEntry::MismatchedSprites {
            a: a.len(),
            b: b.len(),
        });
    }
    let min_len = a.len().min(b.len());
    for i in 0..min_len {
        let name = &a[i].name;
        diff_target_contents(name, a[i].variables.len(), b[i].variables.len(), "variables", result);
        diff_target_contents(name, a[i].lists.len(), b[i].lists.len(), "lists", result);
        diff_target_contents(name, a[i].scripts.len(), b[i].scripts.len(), "scripts", result);
        diff_target_contents(name, a[i].procedures.len(), b[i].procedures.len(), "procedures", result);
    }
}

fn diff_target_contents(name: &str, a_count: usize, b_count: usize, kind: &str, result: &mut DiffResult) {
    if a_count != b_count {
        let entry = match kind {
            "variables" => DiffEntry::MismatchedVariables {
                target: name.to_string(),
                a: a_count,
                b: b_count,
            },
            "lists" => DiffEntry::MismatchedLists {
                target: name.to_string(),
                a: a_count,
                b: b_count,
            },
            "scripts" => DiffEntry::MismatchedScripts {
                target: name.to_string(),
                a: a_count,
                b: b_count,
            },
            "procedures" => DiffEntry::MismatchedProcedures {
                target: name.to_string(),
                a: a_count,
                b: b_count,
            },
            "broadcasts" => DiffEntry::MismatchedBroadcasts {
                a: a_count,
                b: b_count,
            },
            _ => return,
        };
        result.entries.push(entry);
    }
}
