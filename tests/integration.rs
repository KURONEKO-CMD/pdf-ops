use assert_cmd::prelude::*;
// use modern macro form to avoid deprecation warnings
use std::process::Command;
use tempfile::tempdir;
use std::fs;
use std::path::PathBuf;
use lopdf::{Document, Dictionary, Object, ObjectId};

fn create_pdf(dir: &std::path::Path, name: &str, pages: usize) -> PathBuf {
    let mut doc = Document::with_version("1.5");
    let mut page_ids: Vec<ObjectId> = Vec::new();

    for _ in 0..pages {
        let page_id = doc.new_object_id();
        let mut page = Dictionary::new();
        page.set("Type", "Page");
        // Minimal content: empty resources and media box
        page.set("Resources", Dictionary::new());
        page.set("MediaBox", vec![0.into(), 0.into(), 200.into(), 200.into()]);
        doc.objects.insert(page_id, Object::Dictionary(page));
        page_ids.push(page_id);
    }

    let pages_id = doc.new_object_id();
    for &pid in &page_ids {
        let page_obj = doc.objects.get_mut(&pid).unwrap();
        let page_dict = page_obj.as_dict_mut().unwrap();
        page_dict.set("Parent", Object::Reference(pages_id));
    }
    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", "Pages");
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", pages as i64);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.new_object_id();
    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", "Catalog");
    catalog_dict.set("Pages", Object::Reference(pages_id));
    doc.objects.insert(catalog_id, Object::Dictionary(catalog_dict));

    doc.trailer.set("Root", Object::Reference(catalog_id));
    let path = dir.join(name);
    doc.compress();
    doc.save(&path).unwrap();
    path
}

fn page_count(path: &std::path::Path) -> usize {
    let pdf = Document::load(path).unwrap();
    pdf.get_pages().len()
}

#[test]
fn merge_all_and_with_pages_and_filters() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let in_dir = root.join("in");
    fs::create_dir_all(in_dir.join("sub")).unwrap();
    let _a = create_pdf(&in_dir, "a.pdf", 2);
    let _b = create_pdf(&in_dir, "b.pdf", 3);
    let _c = create_pdf(&in_dir.join("sub"), "c.pdf", 4);

    // include only b.pdf using include glob
    let out1 = root.join("out1.pdf");
    Command::new(assert_cmd::cargo::cargo_bin!(env!("CARGO_PKG_NAME")))
        .args(["merge", "-i"]).arg(&in_dir)
        .args(["-o"]).arg(&out1)
        .args(["--include", "b.pdf"])
        .assert().success();
    assert_eq!(page_count(&out1), 3);

    // exclude sub/** and merge all => a + b only
    let out2 = root.join("out2.pdf");
    Command::new(assert_cmd::cargo::cargo_bin!(env!("CARGO_PKG_NAME")))
        .args(["merge", "-i"]).arg(&in_dir)
        .args(["-o"]).arg(&out2)
        .args(["--exclude", "sub/**"])
        .assert().success();
    assert_eq!(page_count(&out2), 2 + 3);

    // pages spec 1-2 applied to each => a(2)->2, b(3)->2 total 4
    let out3 = root.join("out3.pdf");
    Command::new(assert_cmd::cargo::cargo_bin!(env!("CARGO_PKG_NAME")))
        .args(["merge", "-i"]).arg(&in_dir)
        .args(["-o"]).arg(&out3)
        .args(["--pages", "1-2"]) // apply to a,b
        .args(["--exclude", "sub/**"]) // exclude c
        .assert().success();
    assert_eq!(page_count(&out3), 4);
}

#[test]
fn split_defaults_to_each_and_ranges() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let input = create_pdf(root, "in.pdf", 3);
    let out_dir = root.join("out");

    // default to --each
    Command::new(assert_cmd::cargo::cargo_bin!(env!("CARGO_PKG_NAME")))
        .args(["split", "-i"]).arg(&input)
        .args(["-d"]).arg(&out_dir)
        .assert().success();
    // expect 3 files
    let mut count = 0;
    for e in walkdir::WalkDir::new(&out_dir).into_iter().filter_map(Result::ok) {
        if e.file_type().is_file() && e.path().extension().map(|x| x.eq_ignore_ascii_case("pdf")).unwrap_or(false) { count+=1; }
    }
    assert_eq!(count, 3);

    // ranges produces fewer files
    let out_dir2 = root.join("out2");
    Command::new(assert_cmd::cargo::cargo_bin!(env!("CARGO_PKG_NAME")))
        .args(["split", "-i"]).arg(&input)
        .args(["-d"]).arg(&out_dir2)
        .args(["--ranges", "1-2,3-3"]).assert().success();
    let mut count2 = 0;
    for e in walkdir::WalkDir::new(&out_dir2).into_iter().filter_map(Result::ok) {
        if e.file_type().is_file() && e.path().extension().map(|x| x.eq_ignore_ascii_case("pdf")).unwrap_or(false) { count2+=1; }
    }
    assert_eq!(count2, 2);
}
