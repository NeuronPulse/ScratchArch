//! Semantic diff over normalized ScratchGraph projects.
//!
//! Two `Project`s are compared through the [`SemanticNormalizer`]: all block
//! IDs, declaration ordering, and representation noise are erased first, so the
//! diff never reports them. The remaining differences are classified into
//! semantic categories (see `docs/specification/SCRATCH_SEMANTICS.md` §4):
//!
//! - `Added` / `Removed` — an element exists on only one side;
//! - `Moved` — an identical element present on both sides at a different
//!   relative position (only scripts keep observable ordering);
//! - `Changed` — matched content whose leaf content (literals, names, operator
//!   operands) differs;
//! - `ScopeChanged` — a variable/list changed global ↔ local;
//! - `ControlFlowChanged` — nesting, loop bound/condition, or a `Stop` changed;
//! - `RuntimeChanged` — the difference is confined to runtime frame/heap ABI
//!   ops (`EnterFrame`/`PopFrame`/`FrameSet`/`HeapAlloc` and friends).
//!
//! Categories are aggregate: a body whose *only* difference is a deleted dead
//! assignment reports `Changed`; a body that additionally restructures control
//! reports `ControlFlowChanged`. Precedence is `ControlFlowChanged` >
//! `RuntimeChanged` > `Changed`.

use std::cmp::Ordering;

use scratcharch_scratchgraph::ir::{EventHat, ListScope, Stmt, VariableScope};
use scratcharch_scratchgraph::semantic::{
    NormalizedList, NormalizedProcedure, NormalizedProject, NormalizedScript,
    SemanticNormalizer, NormalizedTarget, NormalizedVariable,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFormat {
    Text,
    Json,
}

/// The semantic category of a single difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffCategory {
    Added,
    Removed,
    Changed,
    Moved,
    ScopeChanged,
    ControlFlowChanged,
    RuntimeChanged,
}

impl DiffCategory {
    pub fn label(self) -> &'static str {
        match self {
            DiffCategory::Added => "Added",
            DiffCategory::Removed => "Removed",
            DiffCategory::Changed => "Changed",
            DiffCategory::Moved => "Moved",
            DiffCategory::ScopeChanged => "ScopeChanged",
            DiffCategory::ControlFlowChanged => "ControlFlowChanged",
            DiffCategory::RuntimeChanged => "RuntimeChanged",
        }
    }
}

/// One categorized difference between two projects.
///
/// `location` is the owning target ("Stage", a sprite name) or "project" for
/// project-global content; `subject` names what changed; `detail` carries
/// whatever context the category needs.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffEntry {
    pub category: DiffCategory,
    pub location: String,
    pub subject: String,
    pub detail: String,
}

impl DiffEntry {
    pub fn summary(&self) -> String {
        match self.detail.as_str() {
            "" => format!("[{}] {}: {}", self.category.label(), self.location, self.subject),
            _ => format!(
                "[{}] {}: {} — {}",
                self.category.label(),
                self.location,
                self.subject,
                self.detail
            ),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiffResult {
    pub entries: Vec<DiffEntry>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn count(&self, category: DiffCategory) -> usize {
        self.entries.iter().filter(|e| e.category == category).count()
    }

    pub fn has(&self, category: DiffCategory) -> bool {
        self.count(category) > 0
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
        let mut out = format!("Semantic differences: {}\n", self.entries.len());
        for category in [
            DiffCategory::Added,
            DiffCategory::Removed,
            DiffCategory::Changed,
            DiffCategory::Moved,
            DiffCategory::ScopeChanged,
            DiffCategory::ControlFlowChanged,
            DiffCategory::RuntimeChanged,
        ] {
            let n = self.count(category);
            if n > 0 {
                out.push_str(&format!("  {}: {}\n", category.label(), n));
            }
        }
        for entry in &self.entries {
            out.push_str(&format!("{}. {}\n", 1, entry.summary()));
        }
        let mut numbered: Vec<String> = self.entries.iter().map(|e| e.summary()).collect();
        let mut out = String::new();
        out.push_str(&format!("Semantic differences: {}\n", numbered.len()));
        for category in [
            DiffCategory::Added,
            DiffCategory::Removed,
            DiffCategory::Changed,
            DiffCategory::Moved,
            DiffCategory::ScopeChanged,
            DiffCategory::ControlFlowChanged,
            DiffCategory::RuntimeChanged,
        ] {
            let n = self.count(category);
            if n > 0 {
                out.push_str(&format!("  {}: {}\n", category.label(), n));
            }
        }
        for (i, summary) in numbered.iter_mut().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, summary));
        }
        out
    }

    fn format_json(&self) -> String {
        let items: Vec<String> = self
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{{ \"category\": \"{}\", \"location\": \"{}\", \"subject\": \"{}\", \"detail\": \"{}\" }}",
                    e.category.label(),
                    e.location.replace('"', "'"),
                    e.subject.replace('"', "'"),
                    e.detail.replace('"', "'"),
                )
            })
            .collect();
        format!(
            "{{ \"count\": {}, \"differences\": [{}] }}",
            self.entries.len(),
            items.join(",\n")
        )
    }
}

/// Compare two projects semantically. Ordering of the returned differences is
/// deterministic but not meaningful beyond grouping.
pub fn semantic_diff(a: &scratcharch_scratchgraph::ir::Project, b: &scratcharch_scratchgraph::ir::Project) -> DiffResult {
    let normalizer = SemanticNormalizer::new();
    let na = normalizer.normalize(a);
    let nb = normalizer.normalize(b);
    let mut result = DiffResult::default();

    diff_target(&na.stage, &nb.stage, &mut result);
    diff_sprites(&na.sprites, &nb.sprites, &mut result);
    diff_broadcasts(&na.broadcasts, &nb.broadcasts, &mut result);
    result
}

/// Also exposed for callers that already hold normalized projects.
pub fn semantic_diff_normalized(na: &NormalizedProject, nb: &NormalizedProject) -> DiffResult {
    let mut result = DiffResult::default();
    diff_target(&na.stage, &nb.stage, &mut result);
    diff_sprites(&na.sprites, &nb.sprites, &mut result);
    diff_broadcasts(&na.broadcasts, &nb.broadcasts, &mut result);
    result
}

fn diff_target(a: &NormalizedTarget, b: &NormalizedTarget, result: &mut DiffResult) {
    let location = target_label(a, b);
    diff_variables(&a.variables, &b.variables, &location, result);
    diff_lists(&a.lists, &b.lists, &location, result);
    diff_procedures(&a.procedures, &b.procedures, &location, result);
    diff_scripts(&a.scripts, &b.scripts, &location, result);
}

fn target_label(a: &NormalizedTarget, _b: &NormalizedTarget) -> String {
    if a.name.is_empty() {
        "Stage".to_string()
    } else {
        a.name.clone()
    }
}

fn diff_sprites(a: &[NormalizedTarget], b: &[NormalizedTarget], result: &mut DiffResult) {
    let mut i = 0;
    let mut j = 0;
    while i < a.len() || j < b.len() {
        match (a.get(i), b.get(j)) {
            (Some(sa), Some(sb)) => match sa.name.cmp(&sb.name) {
                Ordering::Equal => {
                    diff_target(sa, sb, result);
                    i += 1;
                    j += 1;
                }
                Ordering::Less => {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Removed,
                        location: "project".to_string(),
                        subject: format!("sprite '{}'", sa.name),
                        detail: String::new(),
                    });
                    i += 1;
                }
                Ordering::Greater => {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Added,
                        location: "project".to_string(),
                        subject: format!("sprite '{}'", sb.name),
                        detail: String::new(),
                    });
                    j += 1;
                }
            },
            (Some(sa), None) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Removed,
                    location: "project".to_string(),
                    subject: format!("sprite '{}'", sa.name),
                    detail: String::new(),
                });
                i += 1;
            }
            (None, Some(sb)) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Added,
                    location: "project".to_string(),
                    subject: format!("sprite '{}'", sb.name),
                    detail: String::new(),
                });
                j += 1;
            }
            (None, None) => break,
        }
    }
}

fn diff_broadcasts(a: &[String], b: &[String], result: &mut DiffResult) {
    let mut i = 0;
    let mut j = 0;
    while i < a.len() || j < b.len() {
        match (a.get(i), b.get(j)) {
            (Some(ma), Some(mb)) => match ma.cmp(mb) {
                Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
                Ordering::Less => {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Removed,
                        location: "project".to_string(),
                        subject: format!("broadcast '{}'", ma),
                        detail: String::new(),
                    });
                    i += 1;
                }
                Ordering::Greater => {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Added,
                        location: "project".to_string(),
                        subject: format!("broadcast '{}'", mb),
                        detail: String::new(),
                    });
                    j += 1;
                }
            },
            (Some(ma), None) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Removed,
                    location: "project".to_string(),
                    subject: format!("broadcast '{}'", ma),
                    detail: String::new(),
                });
                i += 1;
            }
            (None, Some(mb)) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Added,
                    location: "project".to_string(),
                    subject: format!("broadcast '{}'", mb),
                    detail: String::new(),
                });
                j += 1;
            }
            (None, None) => break,
        }
    }
}

// --- variables / lists ----------------------------------------------------

fn variable_rank(s: VariableScope) -> u8 {
    match s {
        VariableScope::Global => 0,
        VariableScope::SpriteLocal => 1,
        VariableScope::Temporary => 2,
    }
}

fn list_rank(s: ListScope) -> u8 {
    match s {
        ListScope::Global => 0,
        ListScope::SpriteLocal => 1,
    }
}

fn diff_variables(a: &[NormalizedVariable], b: &[NormalizedVariable], location: &str, result: &mut DiffResult) {
    let mut i = 0;
    let mut j = 0;
    while i < a.len() || j < b.len() {
        match (a.get(i), b.get(j)) {
            (Some(va), Some(vb)) => {
                if va.name == vb.name {
                    if va.scope == vb.scope {
                        i += 1;
                        j += 1;
                    } else {
                        result.entries.push(DiffEntry {
                            category: DiffCategory::ScopeChanged,
                            location: location.to_string(),
                            subject: format!("variable '{}'", va.name),
                            detail: format!("{} → {}", scope_label(va.scope), scope_label(vb.scope)),
                        });
                        i += 1;
                        j += 1;
                    }
                } else if var_before(va, vb) {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Removed,
                        location: location.to_string(),
                        subject: format!("variable '{}'", va.name),
                        detail: String::new(),
                    });
                    i += 1;
                } else {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Added,
                        location: location.to_string(),
                        subject: format!("variable '{}'", vb.name),
                        detail: String::new(),
                    });
                    j += 1;
                }
            }
            (Some(va), None) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Removed,
                    location: location.to_string(),
                    subject: format!("variable '{}'", va.name),
                    detail: String::new(),
                });
                i += 1;
            }
            (None, Some(vb)) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Added,
                    location: location.to_string(),
                    subject: format!("variable '{}'", vb.name),
                    detail: String::new(),
                });
                j += 1;
            }
            (None, None) => break,
        }
    }
}

fn var_before(a: &NormalizedVariable, b: &NormalizedVariable) -> bool {
    (a.name.as_str(), variable_rank(a.scope)) < (b.name.as_str(), variable_rank(b.scope))
}

fn diff_lists(a: &[NormalizedList], b: &[NormalizedList], location: &str, result: &mut DiffResult) {
    let mut i = 0;
    let mut j = 0;
    while i < a.len() || j < b.len() {
        match (a.get(i), b.get(j)) {
            (Some(la), Some(lb)) => {
                if la.name == lb.name {
                    if la.scope == lb.scope {
                        i += 1;
                        j += 1;
                    } else {
                        result.entries.push(DiffEntry {
                            category: DiffCategory::ScopeChanged,
                            location: location.to_string(),
                            subject: format!("list '{}'", la.name),
                            detail: format!("{} → {}", list_scope_label(la.scope), list_scope_label(lb.scope)),
                        });
                        i += 1;
                        j += 1;
                    }
                } else if list_before(la, lb) {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Removed,
                        location: location.to_string(),
                        subject: format!("list '{}'", la.name),
                        detail: String::new(),
                    });
                    i += 1;
                } else {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Added,
                        location: location.to_string(),
                        subject: format!("list '{}'", lb.name),
                        detail: String::new(),
                    });
                    j += 1;
                }
            }
            (Some(la), None) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Removed,
                    location: location.to_string(),
                    subject: format!("list '{}'", la.name),
                    detail: String::new(),
                });
                i += 1;
            }
            (None, Some(lb)) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Added,
                    location: location.to_string(),
                    subject: format!("list '{}'", lb.name),
                    detail: String::new(),
                });
                j += 1;
            }
            (None, None) => break,
        }
    }
}

fn list_before(a: &NormalizedList, b: &NormalizedList) -> bool {
    (a.name.as_str(), list_rank(a.scope)) < (b.name.as_str(), list_rank(b.scope))
}

fn scope_label(s: VariableScope) -> &'static str {
    match s {
        VariableScope::Global => "global",
        VariableScope::SpriteLocal => "sprite-local",
        VariableScope::Temporary => "temporary",
    }
}

fn list_scope_label(s: ListScope) -> &'static str {
    match s {
        ListScope::Global => "global",
        ListScope::SpriteLocal => "sprite-local",
    }
}

// --- procedures ------------------------------------------------------------

fn diff_procedures(a: &[NormalizedProcedure], b: &[NormalizedProcedure], location: &str, result: &mut DiffResult) {
    let mut i = 0;
    let mut j = 0;
    while i < a.len() || j < b.len() {
        match (a.get(i), b.get(j)) {
            (Some(pa), Some(pb)) => match pa.name.cmp(&pb.name) {
                Ordering::Equal => {
                    if pa.params != pb.params {
                        result.entries.push(DiffEntry {
                            category: DiffCategory::Changed,
                            location: location.to_string(),
                            subject: format!("procedure '{}'", pa.name),
                            detail: "signature changed".to_string(),
                        });
                    }
                    if let Some(kind) = body_diff(&pa.body, &pb.body) {
                        result.entries.push(DiffEntry {
                            category: kind,
                            location: location.to_string(),
                            subject: format!("procedure '{}'", pa.name),
                            detail: "body".to_string(),
                        });
                    }
                    i += 1;
                    j += 1;
                }
                Ordering::Less => {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Removed,
                        location: location.to_string(),
                        subject: format!("procedure '{}'", pa.name),
                        detail: String::new(),
                    });
                    i += 1;
                }
                Ordering::Greater => {
                    result.entries.push(DiffEntry {
                        category: DiffCategory::Added,
                        location: location.to_string(),
                        subject: format!("procedure '{}'", pb.name),
                        detail: String::new(),
                    });
                    j += 1;
                }
            },
            (Some(pa), None) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Removed,
                    location: location.to_string(),
                    subject: format!("procedure '{}'", pa.name),
                    detail: String::new(),
                });
                i += 1;
            }
            (None, Some(pb)) => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Added,
                    location: location.to_string(),
                    subject: format!("procedure '{}'", pb.name),
                    detail: String::new(),
                });
                j += 1;
            }
            (None, None) => break,
        }
    }
}

// --- scripts ---------------------------------------------------------------

/// Diff two script lists. Scripts are matched by full content (hat + canonical
/// body); matched scripts that changed relative position are reported as
/// `Moved`, unmatched ones as `Added`/`Removed`/`Changed`.
fn diff_scripts(a: &[NormalizedScript], b: &[NormalizedScript], location: &str, result: &mut DiffResult) {
    // content keys
    let key = |s: &NormalizedScript| format!("{:?}::{:?}", s.hat, s.body);
    let mut claimed: Vec<bool> = vec![false; b.len()];
    let mut matched: Vec<(usize, usize)> = Vec::new();

    for (i, sa) in a.iter().enumerate() {
        for (j, sb) in b.iter().enumerate() {
            if !claimed[j] && key(sa) == key(sb) {
                claimed[j] = true;
                matched.push((i, j));
                break;
            }
        }
    }

    // Detect moved (relative-order changed) matched scripts. A matched pair
    // participates in an inversion when an earlier matched pair claims a later
    // B position: the two scripts swapped order.
    for (idx, (i, j)) in matched.iter().enumerate() {
        let inverted = matched[..idx].iter().any(|(_, jp)| jp > j);
        if inverted {
            result.entries.push(DiffEntry {
                category: DiffCategory::Moved,
                location: location.to_string(),
                subject: format!("script ({})", hat_desc(&a[*i].hat)),
                detail: "relative script order changed".to_string(),
            });
        }
    }

    // Handle unmatched A scripts: removed, or changed into an unmatched B
    // script with the same hat.
    let mut a_matched = vec![false; a.len()];
    for (i, _) in &matched {
        a_matched[*i] = true;
    }
    for (i, sa) in a.iter().enumerate() {
        if a_matched[i] {
            continue;
        }
        // Look for an unmatched B script with the same hat.
        let mut paired = None;
        for (j, sb) in b.iter().enumerate() {
            if !claimed[j] && sb.hat == sa.hat {
                paired = Some(j);
                break;
            }
        }
        match paired {
            Some(j) => {
                claimed[j] = true;
                let kind = body_diff(&sa.body, &b[j].body).unwrap_or(DiffCategory::Changed);
                result.entries.push(DiffEntry {
                    category: kind,
                    location: location.to_string(),
                    subject: format!("script ({})", hat_desc(&sa.hat)),
                    detail: "body".to_string(),
                });
            }
            None => {
                result.entries.push(DiffEntry {
                    category: DiffCategory::Removed,
                    location: location.to_string(),
                    subject: format!("script ({})", hat_desc(&sa.hat)),
                    detail: String::new(),
                });
            }
        }
    }

    for (j, sb) in b.iter().enumerate() {
        if !claimed[j] {
            result.entries.push(DiffEntry {
                category: DiffCategory::Added,
                location: location.to_string(),
                subject: format!("script ({})", hat_desc(&sb.hat)),
                detail: String::new(),
            });
        }
    }
}

fn hat_desc(hat: &EventHat) -> String {
    match hat {
        EventHat::GreenFlag => "when green flag clicked".to_string(),
        EventHat::KeyPressed(key) => format!("when '{}' key pressed", key),
        EventHat::SpriteClicked => "when this sprite clicked".to_string(),
        EventHat::BroadcastReceived(msg) => format!("when I receive '{}'", msg),
        EventHat::CloneStart => "when I start as a clone".to_string(),
    }
}

// --- body classification ---------------------------------------------------

/// A body difference, or `None` if the canonical bodies are equal.
fn body_diff(a: &[Stmt], b: &[Stmt]) -> Option<DiffCategory> {
    if a == b {
        return None;
    }
    Some(lcs_classify(a, b))
}

/// Longest-common-subsequence over canonical statement equality. The unaligned
/// regions (gaps between the matched pairs) are walked in order and each edit
/// is classified; the most severe category wins.
fn lcs_classify(a: &[Stmt], b: &[Stmt]) -> DiffCategory {
    // dp[i][j] = LCS length of a[i..], b[j..]
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    // Reconstruct one optimal alignment: matched pairs, in order.
    let mut matched: Vec<(usize, usize)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            matched.push((i, j));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1; // a[i] is deleted
        } else {
            j += 1; // b[j] is inserted
        }
    }

    let mut score: Option<DiffCategory> = None;
    let mut prev_i: i64 = -1;
    let mut prev_j: i64 = -1;
    for &(mi, mj) in &matched {
        let start_i = (prev_i + 1) as usize;
        let start_j = (prev_j + 1) as usize;
        flush_gap(&mut score, &a[start_i..mi], &b[start_j..mj]);
        prev_i = mi as i64;
        prev_j = mj as i64;
    }
    let start_i = (prev_i + 1) as usize;
    let start_j = (prev_j + 1) as usize;
    flush_gap(&mut score, &a[start_i..], &b[start_j..]);
    score.unwrap_or(DiffCategory::Changed)
}

/// Classify one maximal unaligned region between matched statements. The two
/// slices share no canonically-equal statement under this alignment, so their
/// contents are paired by position (substitutions) with the tail reported as
/// deletions/insertions.
fn flush_gap(score: &mut Option<DiffCategory>, a_gap: &[Stmt], b_gap: &[Stmt]) {
    let k = a_gap.len().min(b_gap.len());
    for t in 0..k {
        let (sa, sb) = (&a_gap[t], &b_gap[t]);
        let cat = if sa == sb {
            // Canonically equal yet not aligned: the element reordered at this
            // nesting level. A reorder of two leaves is a real change.
            DiffCategory::Changed
        } else {
            classify_sub(sa, sb)
        };
        *score = Some(merge_score(*score, cat));
    }
    for sa in &a_gap[k..] {
        *score = Some(merge_score(*score, edit_category(sa)));
    }
    for sb in &b_gap[k..] {
        *score = Some(merge_score(*score, edit_category(sb)));
    }
}

/// Combine two partial scores with precedence ControlFlowChanged >
/// RuntimeChanged > Changed.
fn control_weight(c: DiffCategory) -> u8 {
    match c {
        DiffCategory::ControlFlowChanged => 3,
        DiffCategory::RuntimeChanged => 2,
        DiffCategory::Changed => 1,
        _ => 0,
    }
}

fn merge_score(a: Option<DiffCategory>, b: DiffCategory) -> DiffCategory {
    match a {
        None => b,
        Some(prev) => {
            if control_weight(b) > control_weight(prev) {
                b
            } else {
                prev
            }
        }
    }
}

fn is_control(s: &Stmt) -> bool {
    matches!(
        s,
        Stmt::If { .. } | Stmt::Repeat { .. } | Stmt::RepeatUntil { .. } | Stmt::Forever { .. } | Stmt::Stop { .. }
    )
}

fn is_runtime(s: &Stmt) -> bool {
    matches!(s, Stmt::EnterFrame { .. } | Stmt::PopFrame { .. } | Stmt::FrameSet { .. } | Stmt::HeapAlloc { .. })
}

/// Category of inserting/removing a single statement.
fn edit_category(s: &Stmt) -> DiffCategory {
    if is_control(s) {
        DiffCategory::ControlFlowChanged
    } else if is_runtime(s) {
        DiffCategory::RuntimeChanged
    } else {
        DiffCategory::Changed
    }
}

/// Classify one aligned substitution of two non-equal statements.
fn classify_sub(a: &Stmt, b: &Stmt) -> DiffCategory {
    match (a, b) {
        (Stmt::If { condition: ca, then_body: ta, else_body: ea }, Stmt::If { condition: cb, then_body: tb, else_body: eb }) => {
            if ca != cb {
                DiffCategory::ControlFlowChanged
            } else {
                nested_merge(body_diff(ta, tb), body_diff(ea, eb))
            }
        }
        (Stmt::Repeat { times: ta, body: ba }, Stmt::Repeat { times: tb, body: bb }) => {
            if ta != tb {
                DiffCategory::ControlFlowChanged
            } else {
                body_diff(ba, bb).unwrap_or(DiffCategory::Changed)
            }
        }
        (Stmt::RepeatUntil { condition: ca, body: ba }, Stmt::RepeatUntil { condition: cb, body: bb }) => {
            if ca != cb {
                DiffCategory::ControlFlowChanged
            } else {
                body_diff(ba, bb).unwrap_or(DiffCategory::Changed)
            }
        }
        (Stmt::Forever { body: ba }, Stmt::Forever { body: bb }) => {
            body_diff(ba, bb).unwrap_or(DiffCategory::Changed)
        }
        (Stmt::Stop { option: oa }, Stmt::Stop { option: ob }) => {
            if oa != ob {
                DiffCategory::ControlFlowChanged
            } else {
                DiffCategory::Changed
            }
        }
        (a, b) if is_control(a) || is_control(b) => DiffCategory::ControlFlowChanged,
        (a, b) if is_runtime(a) || is_runtime(b) => DiffCategory::RuntimeChanged,
        _ => DiffCategory::Changed,
    }
}

fn nested_merge(a: Option<DiffCategory>, b: Option<DiffCategory>) -> DiffCategory {
    match (a, b) {
        (Some(x), Some(y)) => merge_score(Some(x), y),
        (Some(x), None) => x,
        (None, Some(y)) => y,
        (None, None) => DiffCategory::Changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scratcharch_scratchgraph::ir::{Expr, Procedure, Project, Script, Stage, Value};

    fn project(scripts: Vec<Script>, procedures: Vec<Procedure>) -> Project {
        let stage = Stage {
            name: "Stage".to_string(),
            scripts,
            procedures,
            ..Default::default()
        };
        Project {
            stage,
            sprites: vec![],
        }
    }

    fn setv(var: &str, n: f64) -> Stmt {
        Stmt::SetVariable {
            var: var.to_string(),
            value: Expr::number(n),
        }
    }

    fn identical_script() -> Script {
        Script::new(EventHat::GreenFlag, vec![setv("x", 1.0)])
    }

    #[test]
    fn identical_projects_are_empty() {
        let p = project(vec![identical_script()], vec![]);
        assert!(semantic_diff(&p, &p).is_empty());
    }

    #[test]
    fn reordered_sprites_and_vars_are_not_reported() {
        let mut a = Project::new();
        a.stage.name = "Stage".into();
        a.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id-1", "b"));
        a.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id-2", "a"));
        let mut sp_a = scratcharch_scratchgraph::ir::Sprite::new("Cat");
        sp_a.variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id-x", "score"));
        a.sprites.push(sp_a);

        let mut b = Project::new();
        b.stage.name = "Stage".into();
        b.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("other-2", "a"));
        b.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("other-1", "b"));
        let mut sp_b = scratcharch_scratchgraph::ir::Sprite::new("Cat");
        sp_b.variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id-y", "score"));
        b.sprites.push(sp_b);
        assert!(semantic_diff(&a, &b).is_empty());
    }

    #[test]
    fn added_script_is_added() {
        let a = project(vec![], vec![]);
        let b = project(vec![identical_script()], vec![]);
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::Added));
        assert!(d.entries.iter().any(|e| e.subject.contains("green flag")));
    }

    #[test]
    fn removed_variable_is_removed() {
        let mut a = Project::new();
        a.stage.name = "Stage".into();
        a.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id", "x"));
        let b = Project::new();
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::Removed));
        assert!(d.entries.iter().any(|e| e.subject.contains("variable 'x'")));
    }

    #[test]
    fn variable_scope_change_is_scope_changed() {
        use scratcharch_scratchgraph::ir::VariableScope;
        let mut a = Project::new();
        a.stage.name = "Stage".into();
        a.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id", "x").with_scope(VariableScope::Global));
        let mut b = Project::new();
        b.stage.name = "Stage".into();
        b.stage
            .variables
            .push(scratcharch_scratchgraph::ir::Variable::new("id", "x").with_scope(VariableScope::SpriteLocal));
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::ScopeChanged));
        assert!(!d.has(DiffCategory::Added) && !d.has(DiffCategory::Removed));
    }

    #[test]
    fn broadcast_declared_elsewhere_is_equal() {
        use scratcharch_scratchgraph::ir::{Broadcast, Sprite};
        let mut a = Project::new();
        a.add_sprite(Sprite::new("Cat"));
        a.stage.broadcasts.push(Broadcast { id: "a".into(), name: "go".into() });
        let mut b = Project::new();
        b.add_sprite(Sprite {
            name: "Cat".into(),
            broadcasts: vec![Broadcast { id: "z".into(), name: "go".into() }],
            ..Default::default()
        });
        assert!(semantic_diff(&a, &b).is_empty());
    }

    #[test]
    fn body_literal_change_is_changed() {
        let a = project(
            vec![Script::new(EventHat::GreenFlag, vec![setv("x", 1.0)])],
            vec![],
        );
        let b = project(
            vec![Script::new(EventHat::GreenFlag, vec![setv("x", 2.0)])],
            vec![],
        );
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::Changed));
        assert!(!d.has(DiffCategory::ControlFlowChanged));
    }

    #[test]
    fn loop_bound_change_is_control_flow_changed() {
        let mut a = Project::new();
        let mut b = Project::new();
        let body = vec![Stmt::Repeat {
            times: Expr::number(10.0),
            body: vec![setv("x", 1.0)],
        }];
        let body2 = vec![Stmt::Repeat {
            times: Expr::number(20.0),
            body: vec![setv("x", 1.0)],
        }];
        a.stage.scripts.push(Script::new(EventHat::GreenFlag, body));
        b.stage.scripts.push(Script::new(EventHat::GreenFlag, body2));
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::ControlFlowChanged));
    }

    #[test]
    fn added_runtime_frame_op_is_runtime_changed() {
        let base = vec![
            Stmt::EnterFrame { slots: 1 },
            Stmt::SetVariable { var: "acc".into(), value: Expr::number(1.0) },
            Stmt::PopFrame { slots: 1 },
        ];
        let with_extra_frame = vec![
            Stmt::EnterFrame { slots: 1 },
            Stmt::FrameSet { offset: 0, value: Expr::number(0.0) },
            Stmt::SetVariable { var: "acc".into(), value: Expr::number(1.0) },
            Stmt::PopFrame { slots: 1 },
        ];
        let a = project(vec![], vec![Procedure::new("f", vec![], base)]);
        let b = project(vec![], vec![Procedure::new("f", vec![], with_extra_frame)]);
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::RuntimeChanged), "{:?}", d.entries);
        assert!(!d.has(DiffCategory::ControlFlowChanged) && !d.has(DiffCategory::Changed));
    }

    #[test]
    fn script_swap_is_moved() {
        let a = project(
            vec![
                Script::new(EventHat::GreenFlag, vec![setv("x", 1.0)]),
                Script::new(EventHat::GreenFlag, vec![setv("y", 2.0)]),
            ],
            vec![],
        );
        let b = project(
            vec![
                Script::new(EventHat::GreenFlag, vec![setv("y", 2.0)]),
                Script::new(EventHat::GreenFlag, vec![setv("x", 1.0)]),
            ],
            vec![],
        );
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::Moved), "{:?}", d.entries);
    }

    #[test]
    fn removing_broadcast_receiver_is_removed() {
        use scratcharch_scratchgraph::ir::{Broadcast, Sprite};
        let mut a = Project::new();
        a.stage.name = "Stage".into();
        a.stage
            .broadcasts
            .push(Broadcast { id: "i".into(), name: "go".into() });
        a.stage
            .scripts
            .push(Script::new(EventHat::BroadcastReceived("go".into()), vec![setv("x", 1.0)]));
        a.sprites.push(Sprite::new("Cat"));

        // b has the broadcast declared on the sprite instead (same message,
        // same targets) and dropped the receiver script: only the removal
        // should be reported.
        let mut b = Project::new();
        b.stage.name = "Stage".into();
        b.sprites.push(Sprite {
            name: "Cat".into(),
            broadcasts: vec![Broadcast { id: "j".into(), name: "go".into() }],
            ..Default::default()
        });
        let d = semantic_diff(&a, &b);
        assert!(d.has(DiffCategory::Removed), "{:?}", d.entries);
        assert!(!d.has(DiffCategory::Added));
        assert!(d.entries.iter().any(|e| e.subject.contains("receive 'go'")));
    }

    #[test]
    fn value_equal_but_ir_different_number_units() {
        // -0.0 folds to 0.0
        let a = project(vec![Script::new(EventHat::GreenFlag, vec![setv("x", 0.0)])], vec![]);
        let mut b = Project::new();
        b.stage.scripts.push(Script::new(
            EventHat::GreenFlag,
            vec![Stmt::SetVariable {
                var: "x".into(),
                value: Expr::Literal(Value::Number(-0.0)),
            }],
        ));
        assert!(semantic_diff(&a, &b).is_empty());
    }
}
