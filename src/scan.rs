use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use std::sync::{mpsc, Arc, atomic::{AtomicBool, Ordering}};

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub input_dir: PathBuf,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub extra_exclude_paths: Vec<PathBuf>,
    pub max_depth: Option<usize>,
    pub follow_links: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            input_dir: PathBuf::from("."),
            includes: vec![],
            excludes: vec![],
            extra_exclude_paths: vec![],
            max_depth: None,
            follow_links: false,
        }
    }
}

#[allow(dead_code)]
pub fn collect_pdfs(
    input_dir: &Path,
    includes: &[String],
    excludes: &[String],
    extra_exclude_paths: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let cfg = ScanConfig {
        input_dir: input_dir.to_path_buf(),
        includes: includes.to_vec(),
        excludes: excludes.to_vec(),
        extra_exclude_paths: extra_exclude_paths.to_vec(),
        max_depth: None,
        follow_links: false,
    };
    collect_pdfs_cfg(&cfg)
}

fn build_globset(patterns: &[String]) -> Result<GlobSet> {
    if patterns.is_empty() {
        return Ok(GlobSetBuilder::new().build()?);
    }
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        let g = Glob::new(pat).with_context(|| format!("无效的 GLOB: {}", pat))?;
        builder.add(g);
    }
    Ok(builder.build()?)
}

pub fn collect_pdfs_cfg(cfg: &ScanConfig) -> Result<Vec<PathBuf>> {
    let include_set = build_globset(&cfg.includes).with_context(|| "包含规则无效".to_string())?;
    let exclude_set = build_globset(&cfg.excludes).with_context(|| "排除规则无效".to_string())?;

    let mut wd = WalkDir::new(&cfg.input_dir).follow_links(cfg.follow_links);
    if let Some(d) = cfg.max_depth { wd = wd.max_depth(d); }

    let mut out: Vec<PathBuf> = wd
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext.eq_ignore_ascii_case("pdf")).unwrap_or(false))
        .filter(|e| !cfg.extra_exclude_paths.iter().any(|p| e.path() == p))
        .filter(|e| {
            let rel = e.path().strip_prefix(&cfg.input_dir).unwrap_or(e.path());
            let include_ok = if include_set.is_empty() { true } else { include_set.is_match(rel) };
            let exclude_hit = if exclude_set.is_empty() { false } else { exclude_set.is_match(rel) };
            include_ok && !exclude_hit
        })
        .map(|e| e.path().to_owned())
        .collect();

    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn scan_dir_with_spaces_and_pdfs() {
        let td = tempdir().unwrap();
        let root = td.path().join("My Dir With Spaces");
        fs::create_dir_all(&root).unwrap();
        let p = root.join("a b.pdf");
        // create minimal single-page pdf via lopdf
        let mut doc = lopdf::Document::with_version("1.5");
        let page_id = doc.new_object_id();
        let mut page = lopdf::Dictionary::new();
        page.set("Type", "Page");
        page.set("Resources", lopdf::Dictionary::new());
        page.set("MediaBox", vec![0.into(), 0.into(), 200.into(), 200.into()]);
        doc.objects.insert(page_id, lopdf::Object::Dictionary(page));
        let pages_id = doc.new_object_id();
        {
            let page_obj = doc.objects.get_mut(&page_id).unwrap();
            let page_dict = page_obj.as_dict_mut().unwrap();
            page_dict.set("Parent", lopdf::Object::Reference(pages_id));
        }
        let kids: Vec<lopdf::Object> = vec![lopdf::Object::Reference(page_id)];
        let mut pages_dict = lopdf::Dictionary::new();
        pages_dict.set("Type", "Pages");
        pages_dict.set("Kids", lopdf::Object::Array(kids));
        pages_dict.set("Count", 1);
        doc.objects.insert(pages_id, lopdf::Object::Dictionary(pages_dict));
        let catalog_id = doc.new_object_id();
        let mut catalog_dict = lopdf::Dictionary::new();
        catalog_dict.set("Type", "Catalog");
        catalog_dict.set("Pages", lopdf::Object::Reference(pages_id));
        doc.objects.insert(catalog_id, lopdf::Object::Dictionary(catalog_dict));
        doc.trailer.set("Root", lopdf::Object::Reference(catalog_id));
        doc.compress();
        doc.save(&p).unwrap();

        let cfg = ScanConfig { input_dir: root.clone(), includes: vec![], excludes: vec![], extra_exclude_paths: vec![], max_depth: None, follow_links: false };
        let files = collect_pdfs_cfg(&cfg).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], p);
    }
}

pub enum ScanEvent {
    Found(PathBuf),
    Error(String),
    Done,
}

#[derive(Clone)]
pub struct CancelHandle(Arc<AtomicBool>);
impl CancelHandle {
    pub fn cancel(&self) { self.0.store(true, Ordering::Relaxed); }
    pub fn is_canceled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}

pub fn scan_stream(cfg: ScanConfig) -> (mpsc::Receiver<ScanEvent>, CancelHandle) {
    let (tx, rx) = mpsc::channel();
    let cancel = CancelHandle(Arc::new(AtomicBool::new(false)));
    let cancel_clone = CancelHandle(cancel.0.clone());
    std::thread::spawn(move || {
        let include_set = match build_globset(&cfg.includes) {
            Ok(s) => s,
            Err(e) => { let _ = tx.send(ScanEvent::Error(e.to_string())); let _ = tx.send(ScanEvent::Done); return; }
        };
        let exclude_set = match build_globset(&cfg.excludes) {
            Ok(s) => s,
            Err(e) => { let _ = tx.send(ScanEvent::Error(e.to_string())); let _ = tx.send(ScanEvent::Done); return; }
        };
        let mut wd = WalkDir::new(&cfg.input_dir).follow_links(cfg.follow_links);
        if let Some(d) = cfg.max_depth { wd = wd.max_depth(d); }
        for ent in wd.into_iter() {
            if cancel_clone.is_canceled() { break; }
            match ent {
                Ok(e) => {
                    if !e.file_type().is_file() { continue; }
                    let p = e.path();
                    if !p.extension().map(|ext| ext.eq_ignore_ascii_case("pdf")).unwrap_or(false) { continue; }
                    if cfg.extra_exclude_paths.iter().any(|x| p == x) { continue; }
                    let rel = p.strip_prefix(&cfg.input_dir).unwrap_or(p);
                    let include_ok = if include_set.is_empty() { true } else { include_set.is_match(rel) };
                    let exclude_hit = if exclude_set.is_empty() { false } else { exclude_set.is_match(rel) };
                    if include_ok && !exclude_hit {
                        let _ = tx.send(ScanEvent::Found(p.to_path_buf()));
                    }
                }
                Err(e) => {
                    // 忽略不可访问条目的错误，不中断整体扫描
                    // 仅在需要时可发送一次性提示；此处直接跳过
                    let _ = tx.send(ScanEvent::Error(e.to_string()));
                }
            }
        }
        let _ = tx.send(ScanEvent::Done);
    });
    (rx, cancel)
}
