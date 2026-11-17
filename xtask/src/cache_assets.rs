//! Build-time cache acquisition through Git LFS; source checkouts stay unchanged.
use crate::{Result, git, run, verify_reference};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Asset {
    name: String,
    hash: String,
    size: u64,
}

fn assets(root: &Path) -> Result<Vec<Asset>> {
    let repo = root.join("reference/server");
    let mut result = Vec::new();
    for path in git(
        &repo,
        &[
            "ls-tree",
            "-r",
            "--name-only",
            "HEAD",
            "--",
            "Server/data/cache/",
        ],
    )?
    .lines()
    {
        let Some(name) = path.strip_prefix("Server/data/cache/") else {
            continue;
        };
        if name != "main_file_cache.dat2"
            && name
                .strip_prefix("main_file_cache.idx")
                .is_none_or(|suffix| suffix.parse::<u8>().is_err())
        {
            continue;
        }
        let pointer = run(Command::new("git")
            .current_dir(&repo)
            .args(["show", &format!("HEAD:{path}")]))?
        .stdout;
        let text = std::str::from_utf8(&pointer)?;
        let hash = text
            .lines()
            .find_map(|line| line.strip_prefix("oid sha256:"))
            .ok_or("cache LFS SHA256 missing")?
            .to_owned();
        let size = text
            .lines()
            .find_map(|line| line.strip_prefix("size "))
            .ok_or("cache LFS size missing")?
            .parse()?;
        result.push(Asset {
            name: name.to_owned(),
            hash,
            size,
        });
    }
    if !result
        .iter()
        .any(|asset| asset.name == "main_file_cache.dat2")
        || !result
            .iter()
            .any(|asset| asset.name == "main_file_cache.idx255")
    {
        return Err("locked cache is missing its data/reference files".into());
    }
    Ok(result)
}

fn valid(path: &Path, asset: &Asset) -> Result<bool> {
    if !path.is_file() || path.metadata()?.len() != asset.size {
        return Ok(false);
    }
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)) == asset.hash)
}

pub fn directory(root: &Path) -> PathBuf {
    root.join("assets/cache")
}

pub fn manifest(root: &Path) -> Result<Value> {
    Ok(serde_json::to_value(
        assets(root)?
            .into_iter()
            .map(|asset| {
                (
                    asset.name,
                    json!({"sha256": asset.hash, "size": asset.size}),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>(),
    )?)
}

pub fn verify(root: &Path) -> Result<()> {
    verify_reference(root)?;
    for asset in assets(root)? {
        if !valid(&directory(root).join(&asset.name), &asset)? {
            return Err(format!(
                "missing or changed pinned cache asset {}; run cargo xtask cache-fetch",
                asset.name
            )
            .into());
        }
    }
    Ok(())
}

pub fn fetch(root: &Path) -> Result<()> {
    let upstream = verify_reference(root)?;
    let assets = assets(root)?;
    let destination = directory(root);
    fs::create_dir_all(&destination)?;
    let mut missing = Vec::new();
    for asset in &assets {
        if valid(&destination.join(&asset.name), asset)? {
            continue;
        }
        let existing = root.join("assets/visual/cache").join(&asset.name);
        if valid(&existing, asset)? {
            fs::copy(existing, destination.join(&asset.name))?;
            println!("Reused verified {}", asset.name);
        } else {
            missing.push(asset);
        }
    }
    if !missing.is_empty() {
        // Git LFS fetch walks unrelated lazy blobs in a filtered checkout. Request
        // just the pinned object IDs from its public batch API instead.
        let build = root.join("runs/cache-fetch");
        fs::create_dir_all(&build)?;
        let request = build.join("request.json");
        let response = build.join("response.json");
        fs::write(
            &request,
            serde_json::to_vec(&json!({
                "operation": "download", "transfers": ["basic"],
                "objects": missing.iter().map(|asset| json!({"oid": asset.hash, "size": asset.size})).collect::<Vec<_>>()
            }))?,
        )?;
        let endpoint = format!(
            "{}/info/lfs/objects/batch",
            upstream["server"]["url"]
                .as_str()
                .ok_or("missing repository URL")?
        );
        curl(
            Command::new(curl_name())
                .args([
                    "--request",
                    "POST",
                    "--header",
                    "Content-Type: application/vnd.git-lfs+json",
                    "--header",
                    "Accept: application/vnd.git-lfs+json",
                    "--data-binary",
                    &format!("@{}", request.display()),
                    "--output",
                ])
                .arg(&response)
                .arg(endpoint),
        )?;
        let response: Value = serde_json::from_slice(&fs::read(response)?)?;
        let objects = response["objects"]
            .as_array()
            .ok_or("missing LFS objects")?;
        for asset in missing {
            let object = objects
                .iter()
                .find(|object| object["oid"] == asset.hash)
                .ok_or("missing requested LFS object")?;
            let action = &object["actions"]["download"];
            let url = action["href"].as_str().ok_or("missing LFS download URL")?;
            if !url.starts_with("https://") {
                return Err("LFS download requires HTTPS".into());
            }
            let temporary = build.join(format!("{}.partial", asset.name));
            let mut download = Command::new(curl_name());
            download.args(["--output"]).arg(&temporary);
            if let Some(headers) = action["header"].as_object() {
                for (name, value) in headers {
                    download.arg("--header").arg(format!(
                        "{name}: {}",
                        value.as_str().ok_or("invalid LFS header")?
                    ));
                }
            }
            download.arg(url);
            curl(&mut download)?;
            if !valid(&temporary, asset)? {
                return Err(format!("LFS object checksum mismatch for {}", asset.name).into());
            }
            fs::copy(&temporary, destination.join(&asset.name))?;
            fs::remove_file(temporary)?;
            println!("Fetched verified {}", asset.name);
        }
    }
    verify(root)?;
    println!(
        "Pinned cache ready: {} files at {}",
        assets.len(),
        destination.display()
    );
    Ok(())
}

fn curl_name() -> &'static str {
    if cfg!(windows) { "curl.exe" } else { "curl" }
}
fn curl(command: &mut Command) -> Result<()> {
    command.args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "15",
        "--max-time",
        "120",
    ]);
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "cache HTTPS request failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}
