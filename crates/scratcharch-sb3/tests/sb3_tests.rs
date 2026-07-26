use scratcharch_sb3::{
    asset::AssetManager, project::Sb3Project, reader::Sb3Reader, writer::Sb3Writer,
};
use scratcharch_scratchgraph::ir::{Project, Stage, Variable};

fn make_minimal_project() -> Project {
    let stage = Stage::new("Stage");
    Project::new().with_stage(stage)
}

fn make_project_with_vars() -> Project {
    let mut stage = Stage::new("Stage");
    stage.add_variable(Variable::new("myvar", "my variable"));
    let project = Project::new().with_stage(stage);
    project
}

#[test]
fn test_sb3_writer_creates_valid_archive() {
    let project = make_minimal_project();
    let writer = Sb3Writer::new();
    let archive = writer.write(&project);

    assert_eq!(archive.project.targets.len(), 1);
    assert_eq!(archive.project.targets[0].name, "Stage");
    assert!(archive.project.targets[0].is_stage);
}

#[test]
fn test_sb3_roundtrip_memory() {
    // Create a project, write to archive, read back
    let project = make_project_with_vars();
    let writer = Sb3Writer::new();
    let archive = writer.write(&project);

    // Serialize to bytes
    let bytes = archive.to_bytes().expect("serialization failed");

    // Read back
    let archive2 =
        scratcharch_sb3::Sb3Archive::from_bytes(&bytes).expect("deserialization failed");

    assert_eq!(archive2.project.targets.len(), 1);
    assert_eq!(archive2.project.targets[0].name, "Stage");
}

#[test]
fn test_sb3_reader_converts_archive_to_scratchgraph() {
    let project = make_project_with_vars();
    let writer = Sb3Writer::new();
    let archive = writer.write(&project);

    let reader = Sb3Reader::new();
    let result = reader.read(&archive).expect("reader failed");

    assert_eq!(result.stage.name, "Stage");
    assert_eq!(result.stage.variables.len(), 1);
    assert_eq!(result.stage.variables[0].name, "my variable");
}

#[test]
fn test_asset_manager_add_and_retrieve() {
    let mut mgr = AssetManager::new();
    mgr.add("abc123.svg".to_string(), b"<svg></svg>".to_vec());

    assert!(mgr.contains("abc123.svg"));
    assert_eq!(mgr.len(), 1);

    let asset = mgr.get("abc123.svg").expect("asset not found");
    assert_eq!(asset.md5ext, "abc123.svg");
    assert_eq!(asset.data, b"<svg></svg>");
}

#[test]
fn test_sb3_roundtrip_with_variable() {
    let project = make_project_with_vars();
    let writer = Sb3Writer::new();
    let archive = writer.write(&project);
    let bytes = archive.to_bytes().expect("serialization failed");

    let archive2 =
        scratcharch_sb3::Sb3Archive::from_bytes(&bytes).expect("deserialization failed");

    // Check variable was preserved
    let target = &archive2.project.targets[0];
    let vars = target.variables.as_object().unwrap();
    assert!(vars.contains_key("myvar"));
}

#[test]
fn test_project_json_serialization() {
    use serde_json::json;

    let json_val = json!({
        "targets": [{
            "name": "Stage",
            "isStage": true,
            "variables": {},
            "lists": {},
            "broadcasts": {},
            "blocks": {},
            "costumes": [],
            "sounds": []
        }],
        "monitors": [],
        "extensions": [],
        "meta": { "semver": "3.0.0", "vm": "0.2.0", "agent": "test" }
    });

    let project = Sb3Project::from_json(&json_val).expect("parsing failed");
    assert_eq!(project.targets.len(), 1);
    assert!(project.targets[0].is_stage);
    assert_eq!(project.targets[0].name, "Stage");
}

#[test]
fn test_sb3_archive_with_assets() {
    let project = make_minimal_project();
    let writer = Sb3Writer::new();
    let mut archive = writer.write(&project);

    // Add a test asset
    let svg_data = b"<svg xmlns='http://www.w3.org/2000/svg'></svg>".to_vec();
    archive
        .assets
        .add("test_asset.svg".to_string(), svg_data.clone());

    let bytes = archive.to_bytes().expect("serialization failed");
    let archive2 =
        scratcharch_sb3::Sb3Archive::from_bytes(&bytes).expect("deserialization failed");

    assert_eq!(archive2.assets.len(), 1);
    let asset = archive2.assets.get("test_asset.svg").expect("asset missing");
    assert_eq!(asset.data, svg_data);
}
