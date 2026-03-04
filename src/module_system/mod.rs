use crate::args::module::InstallArgs;
use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use regex::Regex;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;

#[derive(Debug, Clone)]
pub enum Source {
    Github { repo: String, tag: Option<String> },
    Url(String),
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct BuildResult {
    pub package_path: PathBuf,
    pub sha512_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    pub schema: String,
    pub modules: Vec<InstalledModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModule {
    pub id: String,
    pub version: String,
    pub min_ect_version: String,
    pub path: String,
    pub sha512: String,
    pub source: String,
    pub unsafe_install: bool,
    pub installed_at: String,
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub id: String,
    pub author: String,
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub platform: Vec<String>,
    pub requirements: Requirements,
    pub aliases: Option<std::collections::BTreeMap<String, AliasEntry>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AliasEntry {
    pub exec: String,
    #[serde(default)]
    pub command: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Requirements {
    pub ect: String,
}

#[derive(Debug, Clone)]
struct ParsedFilename {
    id: String,
    min_ect_version: String,
    module_version: String,
}

pub fn infer_source(
    source: Option<String>,
    url: Option<String>,
    file: Option<String>,
    tag: Option<String>,
) -> Result<Source> {
    let mut count = 0;
    if source.is_some() {
        count += 1;
    }
    if url.is_some() {
        count += 1;
    }
    if file.is_some() {
        count += 1;
    }
    if count != 1 {
        bail!("Use exactly one source: <source> or --url or --file");
    }

    if let Some(u) = url {
        return Ok(Source::Url(u));
    }
    if let Some(f) = file {
        return Ok(Source::File(PathBuf::from(f)));
    }

    let raw = source.expect("checked");
    if raw.starts_with("http://") || raw.starts_with("https://") {
        Ok(Source::Url(raw))
    } else if raw.contains('/') && !raw.contains(std::path::MAIN_SEPARATOR) {
        Ok(Source::Github { repo: raw, tag })
    } else {
        Ok(Source::File(PathBuf::from(raw)))
    }
}

pub fn install_module(source: Source, args: &InstallArgs) -> Result<InstallResult> {
    let (package_bytes, package_name, source_label) = fetch_package(source, args)?;
    let parsed = parse_filename(&package_name, "ectm")?;

    let hash = sha512_hex(&package_bytes);
    if !args.allow_missing_sha512 {
        let sha_text = fetch_sha512_for_package(&source_label, &package_name, args)?;
        validate_sha512(&sha_text, &package_name, &hash)?;
    } else {
        eprintln!("UNSAFE: allowing install without sha512");
    }

    let tmp_dir = std::env::temp_dir().join(format!(
        "ectm-install-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
    ));
    fs::create_dir_all(&tmp_dir)?;
    let package_path = tmp_dir.join(&package_name);
    fs::write(&package_path, &package_bytes)?;

    let extract_dir = tmp_dir.join("extract");
    fs::create_dir_all(&extract_dir)?;
    unzip(&package_path, &extract_dir)?;

    let manifest_path = extract_dir.join("manifest.toml");
    let manifest: Manifest = toml::from_str(&fs::read_to_string(&manifest_path)?)
        .context("Failed to parse manifest.toml")?;

    validate_manifest(&manifest, &parsed)?;
    validate_ect_compat(&manifest.requirements.ect, args.allow_major_compat_mismatch)?;

    let modules_dir = modules_dir()?;
    let target_dir = modules_dir.join(&manifest.id).join(&manifest.version);
    if target_dir.exists() {
        if !args.force {
            bail!("Module version already installed. Use --force to overwrite.");
        }
        fs::remove_dir_all(&target_dir)?;
    }
    fs::create_dir_all(&target_dir)?;

    copy_dir_all(&extract_dir, &target_dir)?;

    let mut registry = load_registry()?;
    registry.schema = "ect.modules.registry.v1".into();
    registry.modules.retain(|m| !(m.id == manifest.id && m.version == manifest.version));
    registry.modules.push(InstalledModule {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        min_ect_version: parsed.min_ect_version,
        path: target_dir.to_string_lossy().into_owned(),
        sha512: hash,
        source: source_label,
        unsafe_install: args.allow_http || args.allow_insecure_tls || args.allow_missing_sha512,
        installed_at: unix_ts_string()?,
        aliases: manifest
            .aliases
            .as_ref()
            .map(|m| m.iter().filter(|(_, a)| a.command).map(|(k, _)| k.clone()).collect()),
    });
    save_registry(&registry)?;

    Ok(InstallResult {
        id: manifest.id,
        version: manifest.version,
    })
}

fn fetch_package(source: Source, args: &InstallArgs) -> Result<(Vec<u8>, String, String)> {
    match source {
        Source::File(path) => {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .context("Invalid file name")?
                .to_string();
            let bytes = fs::read(&path)?;
            Ok((bytes, name, format!("file:{}", path.display())))
        }
        Source::Url(url) => {
            enforce_url_policy(&url, args)?;
            let client = http_client(args)?;
            let resp = client.get(&url).send()?.error_for_status()?;
            let bytes = resp.bytes()?.to_vec();
            let name = url
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("module.ectm")
                .to_string();
            Ok((bytes, name, url))
        }
        Source::Github { repo, tag } => {
            let api = if let Some(tag) = tag {
                format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
            } else {
                format!("https://api.github.com/repos/{repo}/releases/latest")
            };
            let client = reqwest::blocking::Client::builder()
                .user_agent("EnderCliTools")
                .build()?;
            let release: serde_json::Value = client.get(&api).send()?.error_for_status()?.json()?;
            let assets = release["assets"].as_array().context("Missing release assets")?;
            let ectm = assets
                .iter()
                .find(|a| a["name"].as_str().unwrap_or_default().ends_with(".ectm"))
                .context("No .ectm asset found")?;
            let name = ectm["name"].as_str().unwrap_or_default().to_string();
            let dl = ectm["browser_download_url"]
                .as_str()
                .context("Missing asset download url")?;
            let bytes = client.get(dl).send()?.error_for_status()?.bytes()?.to_vec();
            Ok((bytes, name, format!("github:{repo}")))
        }
    }
}

fn fetch_sha512_for_package(source_label: &str, package_name: &str, args: &InstallArgs) -> Result<String> {
    if let Some(path) = source_label.strip_prefix("file:") {
        let mut sha_path = PathBuf::from(path);
        sha_path.set_file_name(package_name.replace(".ectm", ".sha512"));
        return Ok(fs::read_to_string(sha_path)?);
    }

    if let Some(repo) = source_label.strip_prefix("github:") {
        let api = format!("https://api.github.com/repos/{repo}/releases/latest");
        let client = reqwest::blocking::Client::builder()
            .user_agent("EnderCliTools")
            .build()?;
        let release: serde_json::Value = client.get(&api).send()?.error_for_status()?.json()?;
        let expected = package_name.replace(".ectm", ".sha512");
        let assets = release["assets"].as_array().context("Missing release assets")?;
        let sha = assets
            .iter()
            .find(|a| a["name"].as_str().unwrap_or_default() == expected)
            .context("No matching .sha512 asset found")?;
        let url = sha["browser_download_url"]
            .as_str()
            .context("Missing checksum url")?;
        return Ok(client.get(url).send()?.error_for_status()?.text()?);
    }

    // URL
    enforce_url_policy(source_label, args)?;
    let sha_url = if source_label.ends_with(".ectm") {
        source_label.replace(".ectm", ".sha512")
    } else {
        format!("{source_label}.sha512")
    };
    let client = http_client(args)?;
    Ok(client.get(&sha_url).send()?.error_for_status()?.text()?)
}

fn validate_sha512(sha_text: &str, package_name: &str, hash: &str) -> Result<()> {
    let first = sha_text.lines().next().unwrap_or_default().trim();
    let expected = if first.contains(' ') {
        first.split_whitespace().next().unwrap_or_default()
    } else {
        first
    };
    if expected.len() != 128 {
        bail!("Invalid SHA-512 file for {package_name}");
    }
    if !expected.eq_ignore_ascii_case(hash) {
        bail!("Checksum mismatch for {package_name}");
    }
    Ok(())
}

fn parse_filename(name: &str, ext: &str) -> Result<ParsedFilename> {
    let re = Regex::new(&format!(
        r"^([a-z0-9][a-z0-9-]*\.[a-z0-9][a-z0-9_-]*)\.(\d+\.\d+\.\d+)\.(\d+\.\d+\.\d+)\.{ext}$"
    ))?;
    let caps = re
        .captures(name)
        .with_context(|| format!("Filename does not match required scheme: {name}"))?;
    Ok(ParsedFilename {
        id: caps.get(1).unwrap().as_str().to_string(),
        min_ect_version: caps.get(2).unwrap().as_str().to_string(),
        module_version: caps.get(3).unwrap().as_str().to_string(),
    })
}

fn validate_manifest(manifest: &Manifest, parsed: &ParsedFilename) -> Result<()> {
    if manifest.id != parsed.id {
        bail!("Manifest id does not match package filename");
    }

    let split: Vec<&str> = manifest.id.split('.').collect();
    if split.len() != 2 {
        bail!("Manifest id must be author.module_name");
    }
    if manifest.author != split[0] {
        bail!("Manifest author must match id prefix");
    }
    if manifest.name != split[1] {
        bail!("Manifest name must match id suffix");
    }
    if manifest.version != parsed.module_version {
        bail!("Manifest version does not match package filename");
    }

    let _ = Version::parse(&manifest.version)?;
    let _ = Version::parse(&parsed.min_ect_version)?;
    if manifest.platform.is_empty() {
        bail!("Manifest platform must not be empty");
    }

    Ok(())
}

fn validate_ect_compat(req: &str, allow_major: bool) -> Result<()> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let req_norm = req.replace(' ', ", ");
    let ver_req = VersionReq::parse(&req_norm).with_context(|| format!("Invalid requirements.ect: {req}"))?;

    if ver_req.matches(&current) {
        return Ok(());
    }

    if allow_major {
        eprintln!("UNSAFE: allow-major-compat-mismatch is active");
        return Ok(());
    }

    bail!(
        "ECT compatibility mismatch: running {}, required {}",
        current,
        req
    )
}

pub fn load_registry() -> Result<Registry> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(Registry {
            schema: "ect.modules.registry.v1".into(),
            modules: vec![],
        });
    }
    let txt = fs::read_to_string(path)?;
    Ok(toml::from_str(&txt)?)
}

pub fn save_registry(reg: &Registry) -> Result<()> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string_pretty(reg)?)?;
    Ok(())
}

pub fn remove_module(reg: &mut Registry, id: &str, version: Option<&str>) -> Result<InstalledModule> {
    let idx = reg
        .modules
        .iter()
        .position(|m| m.id == id && version.map(|v| v == m.version).unwrap_or(true))
        .context("Module not found")?;
    let removed = reg.modules.remove(idx);
    let path = PathBuf::from(&removed.path);
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(removed)
}

pub fn run_module(id_or_alias: &str, binary: Option<&str>, args: &[String]) -> Result<()> {
    let reg = load_registry()?;

    let module = reg
        .modules
        .iter()
        .find(|m| m.id == id_or_alias)
        .or_else(|| {
            reg.modules.iter().find(|m| {
                m.aliases
                    .as_ref()
                    .map(|a| a.iter().any(|x| x == id_or_alias))
                    .unwrap_or(false)
            })
        })
        .context("Module not installed")?;

    let manifest_path = Path::new(&module.path).join("manifest.toml");
    let manifest: Manifest = toml::from_str(&fs::read_to_string(manifest_path)?)?;

    let platform = current_platform();
    let bin = binary.unwrap_or(&manifest.name);
    let mut path = Path::new(&module.path)
        .join("bin")
        .join(platform)
        .join(bin);

    if cfg!(windows) && path.extension().is_none() {
        path.set_extension("exe");
    }

    if !path.exists() {
        bail!("Module binary not found: {}", path.display());
    }

    let status = Command::new(path).args(args).status()?;
    std::process::exit(status.code().unwrap_or(1));
}

pub fn build_package(path: &str, out_dir: Option<&str>, min_ect: Option<&str>) -> Result<BuildResult> {
    let module_root = PathBuf::from(path);
    let manifest_path = module_root.join("manifest.toml");
    let manifest: Manifest = toml::from_str(&fs::read_to_string(&manifest_path)?)?;

    let min_ect_version = if let Some(v) = min_ect {
        Version::parse(v)?;
        v.to_string()
    } else {
        infer_min_ect_from_req(&manifest.requirements.ect)?
    };

    let parsed = ParsedFilename {
        id: manifest.id.clone(),
        min_ect_version: min_ect_version.clone(),
        module_version: manifest.version.clone(),
    };
    validate_manifest(&manifest, &parsed)?;

    for p in &manifest.platform {
        let bin_dir = module_root.join("bin").join(p);
        if !bin_dir.exists() {
            bail!("Missing platform dir: {}", bin_dir.display());
        }
    }

    let out = out_dir.map(PathBuf::from).unwrap_or_else(|| module_root.join("dist"));
    fs::create_dir_all(&out)?;

    let package_name = format!(
        "{}.{}.{}.ectm",
        manifest.id, min_ect_version, manifest.version
    );
    let package_path = out.join(package_name);

    zip_dir(&module_root, &package_path)?;

    let bytes = fs::read(&package_path)?;
    let hash = sha512_hex(&bytes);
    let sha_name = package_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .replace(".ectm", ".sha512");
    let sha_path = out.join(sha_name);
    fs::write(&sha_path, format!("{}\n", hash))?;

    Ok(BuildResult {
        package_path,
        sha512_path: sha_path,
    })
}

pub fn show_info(target: &str) -> Result<()> {
    if target.ends_with(".ectm") && Path::new(target).exists() {
        let parsed = parse_filename(
            Path::new(target)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default(),
            "ectm",
        )?;
        println!("id: {}", parsed.id);
        println!("min_ect_version: {}", parsed.min_ect_version);
        println!("module_version: {}", parsed.module_version);
        return Ok(());
    }

    let reg = load_registry()?;
    if let Some(m) = reg.modules.iter().find(|m| m.id == target) {
        println!("id: {}", m.id);
        println!("version: {}", m.version);
        println!("source: {}", m.source);
        println!("path: {}", m.path);
        return Ok(());
    }

    bail!("No module info found for target: {target}")
}

fn http_client(args: &InstallArgs) -> Result<reqwest::blocking::Client> {
    let mut builder = reqwest::blocking::Client::builder().user_agent("EnderCliTools");
    if args.allow_insecure_tls {
        eprintln!("UNSAFE: allowing insecure TLS");
        builder = builder.danger_accept_invalid_certs(true);
    }
    Ok(builder.build()?)
}

fn enforce_url_policy(url: &str, args: &InstallArgs) -> Result<()> {
    if url.starts_with("https://") {
        return Ok(());
    }
    if url.starts_with("http://") && args.allow_http {
        eprintln!("UNSAFE: allowing HTTP source");
        return Ok(());
    }
    bail!("Only HTTPS URLs are allowed. Use --allow-http to override.")
}

fn sha512_hex(data: &[u8]) -> String {
    let mut hasher = Sha512::new();
    hasher.update(data);
    let out = hasher.finalize();
    format!("{:x}", out)
}

fn unzip(zip_path: &Path, out_dir: &Path) -> Result<()> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    archive.extract(out_dir)?;
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn modules_dir() -> Result<PathBuf> {
    let proj = ProjectDirs::from("net", "endkind", "enderclitools")
        .context("Could not determine project dirs")?;
    Ok(proj.data_local_dir().join("modules"))
}

fn registry_path() -> Result<PathBuf> {
    let proj = ProjectDirs::from("net", "endkind", "enderclitools")
        .context("Could not determine project dirs")?;
    Ok(proj.config_dir().join("modules.toml"))
}

fn current_platform() -> &'static str {
    if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "linux-x86_64"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "linux-aarch64"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "macos-aarch64"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "macos-x86_64"
    } else {
        "unknown"
    }
}

fn zip_dir(module_root: &Path, package_path: &Path) -> Result<()> {
    let file = fs::File::create(package_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default();

    let mut stack = vec![module_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let p = entry.path();
            let rel = p.strip_prefix(module_root)?.to_string_lossy().replace('\\', "/");
            if rel.starts_with("dist/") || rel == "dist" {
                continue;
            }
            if entry.file_type()?.is_dir() {
                stack.push(p);
            } else {
                zip.start_file(rel, options)?;
                let mut f = fs::File::open(p)?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)?;
                zip.write_all(&buf)?;
            }
        }
    }

    zip.finish()?;
    Ok(())
}

fn infer_min_ect_from_req(req: &str) -> Result<String> {
    let re = Regex::new(r">=\s*(\d+\.\d+\.\d+)")?;
    if let Some(c) = re.captures(req) {
        return Ok(c.get(1).unwrap().as_str().to_string());
    }
    bail!("Could not infer min ect version from requirements.ect; pass --min-ect")
}

fn unix_ts_string() -> Result<String> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(ts.to_string())
}
