use scratcharch_scratchgraph::ir::Project;

use crate::callgraph::CallGraph;
use crate::reachability::UnreachableScript;
use crate::variables::VariableUsageAnalysis;

/// Hash (placeholder) for a project's structure.
///
/// In a future version this would be a content-addressable digest of the
/// project's scripts, procedures, and variable declarations.  For now, a
/// string-based key is sufficient to demonstrate the caching API.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProjectHash(String);

impl ProjectHash {
    /// Compute a hash from the project's structural elements.
    ///
    /// This does **not** hash every block—only the count and ordering of
    /// top-level scripts and procedures.  A full content hash would require
    /// hashing every `Stmt` and `Expr`, which is deferred to a later
    /// milestone.
    pub fn compute(project: &Project) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        project.stage.name.hash(&mut hasher);
        project.stage.scripts.len().hash(&mut hasher);
        project.stage.procedures.len().hash(&mut hasher);
        for sprite in &project.sprites {
            sprite.name.hash(&mut hasher);
            sprite.scripts.len().hash(&mut hasher);
            sprite.procedures.len().hash(&mut hasher);
        }
        ProjectHash(format!("{:x}", hasher.finish()))
    }
}

/// Cached analysis results for a single project.
///
/// `AnalysisCache` stores the output of the most common analyses keyed by
/// a structural hash of the project.  Callers can check
/// [`needs_reanalysis`](Self::needs_reanalysis) to skip re-running
/// expensive passes when the project has not changed.
#[derive(Debug, Clone, Default)]
pub struct AnalysisCache {
    cached_hash: Option<ProjectHash>,
    pub call_graph: Option<CallGraph>,
    pub unreachable: Option<Vec<UnreachableScript>>,
    pub variable_usage: Option<VariableUsageAnalysis>,
}

impl AnalysisCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the given project differs from the cached one.
    pub fn needs_reanalysis(&self, project: &Project) -> bool {
        match &self.cached_hash {
            Some(h) => *h != ProjectHash::compute(project),
            None => true,
        }
    }

    /// Store analysis results and update the cached project hash.
    pub fn store(
        &mut self,
        project: &Project,
        call_graph: CallGraph,
        unreachable: Vec<UnreachableScript>,
        variable_usage: VariableUsageAnalysis,
    ) {
        self.cached_hash = Some(ProjectHash::compute(project));
        self.call_graph = Some(call_graph);
        self.unreachable = Some(unreachable);
        self.variable_usage = Some(variable_usage);
    }

    /// Invalidate all cached results.
    pub fn clear(&mut self) {
        self.cached_hash = None;
        self.call_graph = None;
        self.unreachable = None;
        self.variable_usage = None;
    }
}
