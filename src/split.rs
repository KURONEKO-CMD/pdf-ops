use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::Path;

use crate::spec::{self, PageRange};

pub fn run(input: &Path, out_dir: &Path, each: bool, ranges_spec: Option<&str>, pattern: &str) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("创建输出目录失败 {}: {}", out_dir.display(), e))?;

    let base = input.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let pdf = Document::load(input).map_err(|e| e.to_string())?;
    let total_pages = pdf.get_pages().len();
    if total_pages == 0 { return Err("输入 PDF 没有可用页面".into()); }

    // Determine groups
    let groups: Vec<PageRange> = if each {
        (1..=total_pages).map(|p| PageRange { start: p, end: Some(p) }).collect()
    } else if let Some(spec_str) = ranges_spec {
        spec::parse_spec(spec_str).map_err(|e| e.to_string())?
    } else {
        return Err("请使用 --each 或 --ranges 指定分割方式".into());
    };

    for (idx, g) in groups.iter().enumerate() {
        let start = g.start.max(1);
        let end = g.end.unwrap_or(total_pages).min(total_pages);
        if end < start { continue; }

        let mut out_doc = Document::with_version("1.5");
        let mut page_ids: Vec<ObjectId> = Vec::new();

        // Load fresh copy to avoid side effects
        let mut part_pdf = Document::load(input).map_err(|e| e.to_string())?;
        let offset = out_doc.max_id + 1;
        part_pdf.renumber_objects_with(offset);
        out_doc.max_id = part_pdf.max_id;

        // collect pages in selected range (1-based)
        let pages_map = part_pdf.get_pages();
        for (i, (_, pid)) in pages_map.into_iter().enumerate() {
            let p1 = i + 1; // 1-based
            if p1 >= start && p1 <= end {
                page_ids.push(pid);
            }
        }

        // extend objects (includes resources), then rebuild tree
        out_doc.objects.extend(part_pdf.objects);

        let pages_id = out_doc.new_object_id();
        for &pid in &page_ids {
            let page_obj = out_doc.objects.get_mut(&pid).expect("page not found");
            let page_dict = page_obj.as_dict_mut().expect("page not a dict");
            page_dict.set("Parent", Object::Reference(pages_id));
        }
        let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
        let mut pages_dict = Dictionary::new();
        pages_dict.set("Type", "Pages");
        pages_dict.set("Kids", Object::Array(kids));
        pages_dict.set("Count", page_ids.len() as i64);
        out_doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        let catalog_id = out_doc.new_object_id();
        let mut catalog_dict = Dictionary::new();
        catalog_dict.set("Type", "Catalog");
        catalog_dict.set("Pages", Object::Reference(pages_id));
        out_doc.objects.insert(catalog_id, Object::Dictionary(catalog_dict));

        out_doc.trailer = Dictionary::new();
        out_doc.trailer.set("Root", Object::Reference(catalog_id));
        out_doc.compress();

        let out_name = fill_pattern(pattern, base, start, end, idx + 1);
        let out_path = out_dir.join(out_name);
        if let Some(parent) = out_path.parent() { std::fs::create_dir_all(parent).ok(); }
        out_doc.save(&out_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn fill_pattern(pattern: &str, base: &str, start: usize, end: usize, index: usize) -> String {
    pattern
        .replace("{base}", base)
        .replace("{start}", &start.to_string())
        .replace("{end}", &end.to_string())
        .replace("{index}", &index.to_string())
}
