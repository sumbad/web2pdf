use lopdf::{Dictionary, Document, Object};

pub fn fix_tagged_pdf(doc: &mut Document) -> lopdf::Result<()> {
    let has_struct_tree = super::helpers::find_struct_tree_root(doc).is_some();

    if has_struct_tree {
        println!("✅ StructTreeRoot found, adding MarkInfo...");

        let mut mark_info = Dictionary::new();
        mark_info.set("Marked", true);

        doc.trailer.set("MarkInfo", Object::Dictionary(mark_info));
        doc.trailer.set("Marked", true);

        println!("✅ PDF fixed: now contains /MarkInfo /Marked true");
    } else {
        println!("⚠️ StructTreeRoot missing — nothing to fix");
    }

    Ok(())
}
