use std::path::Path;

use scratcharch_sb3::Sb3Archive;

pub fn run_unpack(input: &str, output_dir: Option<&str>) -> Result<(), String> {
    let archive = Sb3Archive::from_path(input)?;

    let out_dir = output_dir
        .map(|d| d.to_string())
        .unwrap_or_else(|| {
            let p = Path::new(input);
            p.with_extension("").to_string_lossy().to_string()
        });

    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("failed to create output directory {out_dir}: {e}"))?;

    // Write project.json
    let json_value = archive
        .project
        .to_json()
        .map_err(|e| format!("failed to serialize project: {e}"))?;
    let json_str = serde_json::to_string_pretty(&json_value)
        .map_err(|e| format!("failed to format project.json: {e}"))?;
    let json_path = Path::new(&out_dir).join("project.json");
    std::fs::write(&json_path, &json_str)
        .map_err(|e| format!("failed to write {json_path:?}: {e}"))?;

    // Write assets
    let assets_dir = Path::new(&out_dir).join("assets");
    if !archive.assets.is_empty() {
        std::fs::create_dir_all(&assets_dir)
            .map_err(|e| format!("failed to create assets directory: {e}"))?;
        for asset in archive.assets.iter() {
            let asset_path = assets_dir.join(&asset.md5ext);
            std::fs::write(&asset_path, &asset.data)
                .map_err(|e| format!("failed to write asset {}: {e}", asset.md5ext))?;
        }
    }

    let asset_count = archive.assets.len();
    println!("Unpacked {input} -> {out_dir}/");
    println!("  project.json");
    if asset_count > 0 {
        println!("  assets/ ({asset_count} files)");
    }

    Ok(())
}

pub fn run_build(input: &str, output: Option<&str>) -> Result<(), String> {
    let archive = Sb3Archive::from_path(input)?;

    let out_path = output
        .map(|o| o.to_string())
        .unwrap_or_else(|| {
            let p = Path::new(input);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
            format!("{stem}_out.sb3")
        });

    archive
        .to_path(&out_path)
        .map_err(|e| format!("failed to write sb3: {e}"))?;

    println!("Built {input} -> {out_path}");
    Ok(())
}

pub fn run_inspect(input: &str) -> Result<(), String> {
    let archive = Sb3Archive::from_path(input)?;

    println!("SB3 Archive: {input}");
    println!("  Targets: {}", archive.project.targets.len());

    for target in &archive.project.targets {
        let kind = if target.is_stage { "Stage" } else { "Sprite" };
        println!("  [{kind}] {}", target.name);
        println!("    Variables: {}", count_map_keys(&target.variables));
        println!("    Lists: {}", count_map_keys(&target.lists));
        println!("    Broadcasts: {}", count_map_keys(&target.broadcasts));
        println!("    Costumes: {}", target.costumes.len());
        println!("    Sounds: {}", target.sounds.len());

        // Count scripts and procedures from blocks
        if let Some(block_obj) = target.blocks.as_object() {
            let top_level: Vec<_> = block_obj
                .values()
                .filter(|v| {
                    v.as_object()
                        .and_then(|o| o.get("topLevel"))
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false)
                })
                .collect();
            let scripts: Vec<_> = top_level
                .iter()
                .filter(|v| {
                    v.as_object()
                        .and_then(|o| o.get("opcode"))
                        .and_then(|o| o.as_str())
                        .map(|o| {
                            o.starts_with("event_") || o.starts_with("control_start")
                        })
                        .unwrap_or(false)
                })
                .collect();
            println!("    Top-level blocks: {} ({} scripts)", top_level.len(), scripts.len());
        }

        if !target.costumes.is_empty() {
            println!("    Costume list:");
            for c in &target.costumes {
                println!("      - {} ({})", c.name, c.md5ext);
            }
        }
        if !target.sounds.is_empty() {
            println!("    Sound list:");
            for s in &target.sounds {
                println!("      - {} ({})", s.name, s.md5ext);
            }
        }
    }

    println!("  Assets: {}", archive.assets.len());
    for asset in archive.assets.iter() {
        println!("    {} ({} bytes)", asset.md5ext, asset.data.len());
    }

    Ok(())
}

fn count_map_keys(value: &serde_json::Value) -> usize {
    value.as_object().map(|o| o.len()).unwrap_or(0)
}
