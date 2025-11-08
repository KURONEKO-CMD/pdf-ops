use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

use crate::spec;
use crate::progress::ProgressSink;
use crate::scan::{self, ScanConfig};

pub fn run(
    input_dir: &Path,
    output: &Path,
    pages_spec: Option<&str>,
    includes: &[String],
    excludes: &[String],
    force: bool,
    progress: &dyn ProgressSink,
) -> Result<()> {
    // Resolve output directory
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建输出目录失败: {}", parent.display()))?;
    }

    // Scan pdf files (reuse scanner) — CLI uses infinite depth by default
    let cfg = ScanConfig {
        input_dir: input_dir.to_path_buf(),
        includes: includes.to_vec(),
        excludes: excludes.to_vec(),
        extra_exclude_paths: vec![output.to_path_buf()],
        max_depth: None,
        follow_links: false,
    };
    let pdf_files = scan::collect_pdfs_cfg(&cfg)?;

    if pdf_files.is_empty() {
        anyhow::bail!("未在目录中找到 PDF: {}", input_dir.display());
    }
    progress.set_len(pdf_files.len() as u64);
    progress.set_message(std::borrow::Cow::from("准备合并..."));
    merge_selected_pages(&pdf_files, output, pages_spec, progress, force)?;
    progress.finish(std::borrow::Cow::from("合并完成"));
    Ok(())
}

pub(crate) fn merge_selected_pages(files: &[PathBuf], output: &Path, pages_spec: Option<&str>, progress: &dyn ProgressSink, force: bool) -> Result<()> {
    // Overwrite protection handled here to ensure we fail early
    if output.exists() && !force {
        anyhow::bail!("输出文件已存在: {} (使用 --force 覆盖)", output.display());
    }
    let mut doc = Document::with_version("1.5");
    let mut page_ids: Vec<ObjectId> = Vec::new();

    for path in files {
        let msg = path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "加载中...".to_string());
        progress.set_message(std::borrow::Cow::from(msg));
        let mut pdf = Document::load(path)
            .with_context(|| format!("加载 PDF 失败: {}", path.display()))?;
        let total_pages = pdf.get_pages().len();
        let indices: Option<Vec<usize>> = if let Some(spec_str) = pages_spec {
            let ranges = spec::parse_spec(spec_str)
                .with_context(|| format!("解析页码范围失败: {}", spec_str))?;
            Some(spec::expand_to_indexes(&ranges, total_pages))
        } else { None };

        let offset = doc.max_id + 1;
        pdf.renumber_objects_with(offset);
        doc.max_id = pdf.max_id;

        let pages_map = pdf.get_pages();
        // Collect in natural order
        let mut current: Vec<ObjectId> = Vec::new();
        for (i, (_, pid)) in pages_map.into_iter().enumerate() {
            if let Some(ref idxs) = indices {
                if !idxs.contains(&i) { continue; }
            }
            current.push(pid);
        }
        page_ids.extend(current);
        doc.objects.extend(pdf.objects);
        progress.inc(1);
    }

    let pages_id = doc.new_object_id();
    for &pid in &page_ids {
        let page_obj = doc
            .objects
            .get_mut(&pid)
            .ok_or_else(|| anyhow::anyhow!("页面对象不存在: {:?}", pid))?;
        match page_obj.as_dict_mut() {
            Ok(page_dict) => {
                page_dict.set("Parent", Object::Reference(pages_id));
            }
            Err(_) => {
                anyhow::bail!("页面对象不是字典: {:?}", pid);
            }
        }
    }
    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", "Pages");
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", page_ids.len() as i64);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.new_object_id();
    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", "Catalog");
    catalog_dict.set("Pages", Object::Reference(pages_id));
    doc.objects.insert(catalog_id, Object::Dictionary(catalog_dict));

    doc.trailer = Dictionary::new();
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.compress();
    doc.save(output)
        .with_context(|| format!("写入输出失败: {}", output.display()))?;
    Ok(())
}

pub fn run_with_files(files: &[PathBuf], output: &Path, pages_spec: Option<&str>, force: bool, progress: &dyn ProgressSink) -> Result<()> {
    merge_selected_pages(files, output, pages_spec, progress, force)
}

// scanner helpers moved to crate::scan
