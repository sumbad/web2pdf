//! PDF fixtures for unit tests. Only compiled with `cargo test`.

use lopdf::{Document, Object, dictionary};

/// Minimal document with a Catalog/Pages tree and `page_count` pages.
///
/// With `tagged` it also carries a structure tree:
/// StructTreeRoot → StructElem "Document" → one StructElem "P" per page,
/// each holding an MCR bound to its page, plus a ParentTree with one Nums
/// entry per paragraph and `ParentTreeNextKey` = number of paragraphs.
pub(crate) fn build_minimal_pdf(page_count: usize, tagged: bool) -> Document {
    let mut doc = Document::with_version("1.7");

    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => Object::Array(Vec::new()),
        "Count" => 0u32,
    });

    let mut page_ids = Vec::new();
    for _ in 0..page_count {
        let id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
        });
        page_ids.push(id);
    }
    let kids: Vec<Object> = page_ids.iter().map(|&id| Object::Reference(id)).collect();
    if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(pages_id) {
        dict.set("Kids", kids);
        dict.set("Count", page_count as u32);
    }

    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };

    if tagged {
        let mut para_ids = Vec::new();
        for &pg in &page_ids {
            let mcr_id = doc.add_object(dictionary! {
                "Type" => "MCR",
                "Pg" => pg,
                "MCID" => 0i64,
            });
            let para_id = doc.add_object(dictionary! {
                "Type" => "StructElem",
                "S" => "P",
                "Pg" => pg,
                "K" => mcr_id,
            });
            para_ids.push(para_id);
        }

        let doc_node_id = doc.add_object(dictionary! {
            "Type" => "StructElem",
            "S" => "Document",
            "K" => Object::Array(para_ids.iter().map(|&id| Object::Reference(id)).collect()),
        });

        for &para in &para_ids {
            if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(para) {
                dict.set("P", doc_node_id);
            }
        }

        let mut nums = Vec::new();
        for (i, &para) in para_ids.iter().enumerate() {
            nums.push(Object::Integer(i as i64));
            nums.push(Object::Reference(para));
        }
        let parent_tree_id = doc.add_object(dictionary! {
            "Nums" => nums,
        });

        let struct_root_id = doc.add_object(dictionary! {
            "Type" => "StructTreeRoot",
            "K" => doc_node_id,
            "ParentTree" => parent_tree_id,
            "ParentTreeNextKey" => para_ids.len() as i64,
        });

        catalog.set("StructTreeRoot", struct_root_id);

        for &pg in &page_ids {
            if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(pg) {
                dict.set("StructParents", 0i64);
            }
        }
    }

    let catalog_id = doc.add_object(catalog);
    doc.trailer = dictionary! {
        "Root" => catalog_id,
        "Size" => (doc.objects.len() as i64) + 1,
    };
    doc.max_id = doc.objects.keys().map(|id| id.0).max().unwrap_or(0);

    doc
}
