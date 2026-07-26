use scratcharch_scratchgraph::SourceMapBuilder;

#[test]
fn test_source_map_builder_empty_project() {
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
    assert!(builder.source_map[0].blocks.is_empty());
}

#[test]
fn test_source_map_with_blocks() {
    let json = serde_json::json!({
        "targets": [{
            "isStage": true,
            "name": "Stage",
            "variables": {},
            "lists": {},
            "broadcasts": {},
            "blocks": {
                "abc123": {
                    "opcode": "event_whenflagclicked",
                    "topLevel": true,
                    "next": null,
                    "parent": null,
                    "inputs": {},
                    "fields": {}
                },
                "def456": {
                    "opcode": "data_setvariableto",
                    "topLevel": false,
                    "parent": "abc123",
                    "next": null,
                    "inputs": {
                        "VALUE": [1, [4, "10"]]
                    },
                    "fields": {
                        "VARIABLE": ["x", null]
                    }
                }
            }
        }]
    });

    let mut builder = SourceMapBuilder::new();
    let project = builder.parse(&json).expect("parse failed");
    assert_eq!(builder.source_map.len(), 1);
    let stage_map = &builder.source_map[0];

    // Should have 2 blocks in the source map
    assert_eq!(stage_map.blocks.len(), 2);

    let flag = stage_map.blocks.iter().find(|b| b.block_id == "abc123").unwrap();
    assert_eq!(flag.opcode, "event_whenflagclicked");
    assert!(flag.is_top_level);
    assert!(flag.parent.is_none());

    let set = stage_map.blocks.iter().find(|b| b.block_id == "def456").unwrap();
    assert_eq!(set.opcode, "data_setvariableto");
    assert!(!set.is_top_level);
    assert_eq!(set.parent, Some("abc123".to_string()));

    // The project should have 1 script (green flag)
    assert_eq!(project.stage.scripts.len(), 1);
}
