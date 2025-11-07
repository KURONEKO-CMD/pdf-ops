use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::spec;

pub fn run(input_dir: &Path, output: &Path, pages_spec: Option<&str>) -> Result<(), String> {
    // Resolve output directory
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建输出目录失败 {}: {}", parent.display(), e))?;
    }

    // Scan pdf files
    let mut pdf_files: Vec<_> = WalkDir::new(input_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext.eq_ignore_ascii_case("pdf")).unwrap_or(false))
        .filter(|e| e.path() != output)
        .map(|e| e.path().to_owned())
        .collect();
    pdf_files.sort();

    if pdf_files.is_empty() {
        return Err(format!("未在目录中找到 PDF: {}", input_dir.display()));
    }

    merge_selected_pages(&pdf_files, output, pages_spec).map_err(|e| e.to_string())?;
    Ok(())
}

fn merge_selected_pages(files: &[PathBuf], output: &Path, pages_spec: Option<&str>) -> lopdf::Result<()> {
    let mut doc = Document::with_version("1.5");
    let mut page_ids: Vec<ObjectId> = Vec::new();

    for path in files {
        let mut pdf = Document::load(path)?;
        let total_pages = pdf.get_pages().len();
        let indices: Option<Vec<usize>> = if let Some(spec_str) = pages_spec {
            let ranges = spec::parse_spec(spec_str).map_err(|e| lopdf::Error::IO(std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string())))?;
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
    }

    let pages_id = doc.new_object_id();
    for &pid in &page_ids {
        let page_obj = doc.objects.get_mut(&pid).expect("page not found");
        let page_dict = page_obj.as_dict_mut().expect("page not a dict");
        page_dict.set("Parent", Object::Reference(pages_id));
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
    doc.save(output)?;
    Ok(())
}
