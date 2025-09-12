use clap::Parser;
use lopdf::{Dictionary, Document, Object, ObjectId};
use walkdir::WalkDir;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about = "Merge all PDF files in a folder")]
struct Cli {
    #[arg(short, long, value_name = "DIR", default_value = ".")]
    input_dir: String,
    #[arg(short, long, value_name = "FILE", default_value = "merged.pdf")]
    output: String,
}

fn main() {
    let cli = Cli::parse();
    let dir = &cli.input_dir;
    let mut output_path = std::path::PathBuf::from(&cli.output);

    if cli.output == "merged.pdf" {
        output_path = std::path::PathBuf::from(&cli.input_dir);
        output_path.push("merged.pdf");
    }


    let mut pdf_files: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_owned())
        .collect();

    pdf_files.sort();

    if pdf_files.is_empty() {
        println!("No PDFs found in {}", dir);
        return;
    }

    if  let Err(e) = merge_pdfs(&pdf_files, output_path.to_str().unwrap()) {
        eprint!("❌ PDF 合并失败：{e}");
        std::process::exit(1);
    }
    println!("✅ 已合并 {} 个 PDF -> {}", pdf_files.len(), output_path.display());
}

fn merge_pdfs(files: &[PathBuf], output: &str) -> lopdf::Result<()> {
    let mut doc = Document::with_version("1.5");
    let mut page_ids: Vec<ObjectId> = Vec::new();

    for path in files {
        let mut pdf = Document::load(path)?;
        let offset = doc.max_id + 1;
        pdf.renumber_objects_with(offset);
        doc.max_id = pdf.max_id;

        for (_, pid) in pdf.get_pages() {
            page_ids.push(pid);
        }
        doc.objects.extend(pdf.objects);
    }
    // 合并对象（不要合并 trailer，避免 Root/Info 冲突）
    let pages_id = doc.new_object_id();

    for &pid in &page_ids {
        let page_obj = doc.objects.get_mut(&pid).expect("page not found");
        let page_dict = page_obj.as_dict_mut().expect("page not a dict");
        page_dict.set("Parent", Object::Reference(pages_id));
    }
    // 组装 Kids 数组
    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
    // /Pages
    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", "Pages");
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", page_ids.len() as i64);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // /Catalog
    let catalog_id = doc.new_object_id();
    let mut catalog_dict = Dictionary::new();
    catalog_dict.set("Type", "Catalog");
    catalog_dict.set("Pages", Object::Reference(pages_id));
    doc.objects
        .insert(catalog_id, Object::Dictionary(catalog_dict));

    // 3) 重建 trailer，只设置新的 Root
    doc.trailer = Dictionary::new();
    doc.trailer.set("Root", Object::Reference(catalog_id));
    doc.compress();
    doc.save(output)?;
    Ok(())
}
