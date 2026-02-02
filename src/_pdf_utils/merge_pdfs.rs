use lopdf::{dictionary, Bookmark, Dictionary, Document, Object, ObjectId};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use super::sanitize_pdf::sanitize_pdf;
use crate::toc::TocNode;

/// Result of processing the structure of a single document
pub struct DocStructureData {
    /// Shifted ParentTree elements (key-value pairs for the Nums array)
    pub shifted_nums: Vec<Object>,
    /// Array of structure children (already "flattened")
    pub root_kids: Vec<Object>,
    /// Dictionary mapping custom tags (RoleMap)
    pub role_map: Option<Dictionary>,
    /// How much to shift the offset for the next document
    pub next_offset_increment: i64,
}

pub fn merge_pdfs<P>(toc: Vec<TocNode>, output: P) -> lopdf::Result<()>
where
    P: AsRef<Path>,
{
    let toc_iter = toc.into_iter();

    // 📌 Step 1.1: Use version 1.7 to support modern Tagged PDF
    let mut document = Document::with_version("1.7");

    let mut max_id = 1;
    let mut pagenum = 1;

    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();

    // 📌 Step 1.2: Collectors for structural data (Stage 1)
    // We'll save each document's StructTreeRoot as separate objects for later analysis
    let mut source_struct_roots = Vec::new();

    let mut previous_lever_bookmark: HashMap<u8, Option<u32>> = HashMap::new();

    let mut global_nums = Vec::new();
    let mut global_kids = Vec::new();
    let mut global_role_map = Dictionary::new();
    let mut current_offset = 0i64;

    for node in toc_iter {
        let file_path = if let Some(path) = node.file_path.as_ref() {
            path
        } else {
            continue;
        };
        let title = node
            .title
            .clone()
            .unwrap_or_else(|| file_path.to_string_lossy().to_string());

        tracing::info!(target: "pdf_merge", "--- Stage 1: Processing file: {:?} ---", file_path);

        let mut doc = match Document::load(file_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("⚠️ Skipping corrupted PDF {:?}: {:?}", file_path, e);
                continue;
            }
        };

        if let Err(e) = sanitize_pdf(&mut doc) {
            tracing::error!(target: "pdf_merge", "Failed to sanitize PDF UA structure: {:?}", e);
        }

        // 📌 Renumbering
        let start_id = max_id;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        tracing::debug!(
            target: "pdf_merge",
            "Renumbered objects for '{}': IDs shifted from {} to {}",
            title, start_id, doc.max_id
        );

        // 📌 Step 1.3: Extract StructTreeRoot data
        // Find the structure root in the current document
        let mut struct_found = false;
        if let Ok(catalog) = doc.catalog() {
            if let Ok(struct_root_res) = catalog.get(b"StructTreeRoot") {
                // Save a reference to this document's StructTreeRoot for stages 2-4
                if let Ok(id) = struct_root_res.as_reference() {
                    if let Ok(dict) = doc.get_object(id).and_then(|o| o.as_dict()) {
                        // Clone the dictionary since doc will be consumed or destroyed
                        source_struct_roots.push(dict.clone());

                        struct_found = true;

                        // Log the keys present in the structure (K, ParentTree, RoleMap, etc.)
                        let keys: Vec<String> = dict
                            .iter()
                            .map(|(k, _)| String::from_utf8_lossy(k).into_owned())
                            .collect();
                        tracing::debug!(target: "pdf_merge", "Found StructTreeRoot (ID: {:?}) with keys: {:?}", id, keys);
                    }
                }
            }
        }

        if !struct_found {
            tracing::warn!(target: "pdf_merge", "No StructTreeRoot found in '{}'. This document might not be Tagged (PDF/UA).", title);
        }

        // --- Call structure processing function ---
        let struct_data = extract_and_shift_structure(&mut doc, current_offset);

        // 1. Collect Nums (ParentTree)
        global_nums.extend(struct_data.shifted_nums);

        // 2. Collect children (K) - now just extend, without if let Some
        global_kids.extend(struct_data.root_kids);

        // 3. Collect RoleMap
        if let Some(rm) = struct_data.role_map {
            for (k, v) in rm {
                global_role_map.set(k.clone(), v.clone());
            }
        }

        // 4. Update global offset for the next file
        current_offset += struct_data.next_offset_increment;

        tracing::debug!(
            target: "pdf_merge",
            "Processed structure for '{}': Shifted {} Nums, incremented offset by {}",
            title, global_nums.len() / 2, struct_data.next_offset_increment
        );

        // 📑 Collect pages and objects
        let mut file_page_count = 0;
        let mut is_first_page = true;
        for (_page_num, object_id) in doc.get_pages() {
            if is_first_page {
                let bookmark =
                    Bookmark::new(title.clone(), [0.0, 0.0, 1.0], pagenum - 1, object_id);
                if node.level == 0 {
                    previous_lever_bookmark.clear();
                }
                let parent = previous_lever_bookmark
                    .get(&node.level.saturating_sub(1))
                    .copied()
                    .flatten();
                previous_lever_bookmark
                    .insert(node.level, Some(document.add_bookmark(bookmark, parent)));
                is_first_page = false;
            }
            pagenum += 1;

            // Important: save the page
            if let Ok(obj) = doc.get_object(object_id) {
                documents_pages.insert(object_id, obj.to_owned());
                file_page_count += 1;
            }
        }

        tracing::debug!(target: "pdf_merge", "Collected {} pages from '{}'. Current total pagenum: {}", file_page_count, title, pagenum + file_page_count - 1);

        // Consume all objects from the current document
        documents_objects.extend(doc.objects);
    }

    tracing::info!(
        target: "pdf_merge",
        "Stage 1 complete: Total objects: {}, Total pages: {}, Struct roots collected: {}",
        documents_objects.len(), documents_pages.len(), source_struct_roots.len()
    );

    ////////////////////////////////////////////////////////////////////////////////////////////////////
    // --- STAGE 4: Determine base IDs and synchronization ---

    // ⚠️ CRITICAL: Synchronize the ID counter in the new document with what we counted in the loop
    document.max_id = max_id;

    let mut catalog_id: Option<ObjectId> = None;
    let mut pages_id: Option<ObjectId> = None;

    // First, simply transfer all common objects (fonts, resources)
    for (id, obj) in &documents_objects {
        match obj.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_id.is_none() {
                    catalog_id = Some(*id);
                    // ⚠️ MANDATORY: insert into the document so assemble_merged_document can find it via get_mut
                    document.objects.insert(*id, obj.clone());
                }
            }
            b"Pages" => {
                if pages_id.is_none() {
                    pages_id = Some(*id);
                    // ⚠️ MANDATORY: insert into the document
                    document.objects.insert(*id, obj.clone());
                }
            }
            b"Page" | b"Outlines" | b"Outline" | b"StructTreeRoot" => {
                // These types we reassemble manually, skip them
            }
            _ => {
                document.objects.insert(*id, obj.clone());
            }
        }
    }

    let catalog_id = catalog_id.expect("Catalog not found");
    let pages_id = pages_id.expect("Pages root not found");

    // --- STAGE 5: Final assembly ---
    // Now assemble_merged_document will get IDs starting from max_id + 1 (i.e., from 370+)
    let mut document = assemble_merged_document(
        document,
        catalog_id,
        pages_id,
        documents_pages,
        global_kids,
        global_nums,
        global_role_map,
        current_offset,
    )?;

    // --- FINALIZATION ---
    document.trailer = dictionary! {
        "Root" => catalog_id,
        "Size" => (document.objects.len() as i64) + 1
    };

    // ⚠️ Shift max_id to the actual value after adding new structure objects
    document.max_id = document
        .objects
        .keys()
        .map(|id| id.0)
        .max()
        .unwrap_or(max_id);

    document.adjust_zero_pages();

    if !document.bookmarks.is_empty() {
        if let Some(outline_id) = document.build_outline() {
            if let Ok(Object::Dictionary(dict)) = document.get_object_mut(catalog_id) {
                dict.set("Outlines", Object::Reference(outline_id));
            }
        }
    }

    // ⚠️ HIGHLY RECOMMENDED: renumber all objects at the very end for "clean" xref table
    document.renumber_objects();

    document.compress();
    document.save(output)?;

    tracing::info!("Merged PDF saved successfully.");
    Ok(())
}

fn extract_and_shift_structure(doc: &mut Document, current_offset: i64) -> DocStructureData {
    let mut shifted_nums = Vec::new();
    let mut root_kids = Vec::new();
    let mut role_map = None;
    let mut local_next_key = 0i64;

    // 1. Пытаемся получить StructTreeRoot через Каталог
    if let Ok(catalog) = doc.catalog() {
        if let Ok(str_root_ref) = catalog
            .get(b"StructTreeRoot")
            .and_then(|o| o.as_reference())
        {
            if let Ok(str_root) = doc.get_object(str_root_ref).and_then(|o| o.as_dict()) {
                // --- A. Get ParentTreeNextKey to calculate future offset ---
                local_next_key = str_root
                    .get(b"ParentTreeNextKey")
                    .and_then(|o| o.as_i64())
                    .unwrap_or(0);

                // --- B. Shift keys in ParentTree (Nums) ---
                if let Ok(pt_ref) = str_root.get(b"ParentTree").and_then(|o| o.as_reference()) {
                    if let Ok(pt_dict) = doc.get_object(pt_ref).and_then(|o| o.as_dict()) {
                        if let Ok(nums) = pt_dict.get(b"Nums").and_then(|o| o.as_array()) {
                            for i in (0..nums.len()).step_by(2) {
                                if let (Some(Object::Integer(k)), Some(val)) =
                                    (nums.get(i), nums.get(i + 1))
                                {
                                    let new_key = k + current_offset;
                                    shifted_nums.push(Object::Integer(new_key));
                                    shifted_nums.push(val.clone());
                                }
                            }
                        }
                    }
                }

                // --- C. Extract and flatten structure children (K) ---
                if let Ok(k_obj) = str_root.get(b"K") {
                    match k_obj {
                        Object::Array(arr) => {
                            root_kids.extend(arr.iter().cloned());
                        }
                        Object::Reference(id) => {
                            // Check: is this object a "Document" type node
                            let is_doc_node = doc
                                .get_object(*id)
                                .ok()
                                .and_then(|o| o.as_dict().ok())
                                .and_then(|d| d.get(b"S").ok())
                                .and_then(|s| s.as_name().ok())
                                == Some(b"Document");

                            if is_doc_node {
                                // If it's a Document, take its children (/K) directly
                                if let Ok(inner_k) =
                                    doc.get_object(*id).and_then(|o| o.as_dict()?.get(b"K"))
                                {
                                    match inner_k {
                                        Object::Array(arr) => root_kids.extend(arr.iter().cloned()),
                                        _ => root_kids.push(inner_k.clone()),
                                    }
                                }
                            } else {
                                // If it's not a Document (e.g., Div or Part), just add the reference
                                root_kids.push(k_obj.clone());
                            }
                        }
                        _ => root_kids.push(k_obj.clone()),
                    }
                }

                // --- D. Extract RoleMap ---
                role_map = str_root
                    .get(b"RoleMap")
                    .ok()
                    .and_then(|o| o.as_dict().ok())
                    .map(|d| d.clone());
            }
        }
    }

    // --- E. Shift StructParents on pages (most important for linking) ---
    for (_page_num, page_id) in doc.get_pages() {
        if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(old_sp) = page_dict.get(b"StructParents").and_then(|o| o.as_i64()) {
                page_dict.set("StructParents", old_sp + current_offset);
            }
        }
    }

    // Calculate increment: how many indices this document occupied.
    // Take the maximum between NextKey and the actual number of pages.
    let page_count = doc.get_pages().len() as i64;
    let increment = local_next_key.max(page_count).max(1);

    DocStructureData {
        shifted_nums,
        root_kids,
        role_map,
        next_offset_increment: increment,
    }
}

fn assemble_merged_document(
    mut document: Document,
    catalog_id: ObjectId,
    pages_id: ObjectId,
    documents_pages: BTreeMap<ObjectId, Object>,
    global_kids: Vec<Object>,
    global_nums: Vec<Object>,
    global_role_map: Dictionary,
    final_offset: i64,
) -> lopdf::Result<Document> {
    tracing::info!(target: "pdf_merge", "--- Stage 4: Assembling final document structure ---");

    // 1. Insert pages into the final document and link them to the new Pages ID
    for (id, obj) in &documents_pages {
        if let Ok(dict) = obj.as_dict() {
            let mut dict = dict.clone();
            dict.set("Parent", pages_id);
            document.objects.insert(*id, Object::Dictionary(dict));
        }
    }
    tracing::debug!(target: "pdf_merge", "Linked {} pages to the new Pages root (ID: {:?})", documents_pages.len(), pages_id);

    // 2. Create a unified ParentTree object (Nums)
    let parent_tree_id = document.add_object(dictionary! {
        "Nums" => global_nums.clone(),
    });
    tracing::debug!(target: "pdf_merge", "Created ParentTree (ID: {:?}) with {} entries", parent_tree_id, global_nums.len() / 2);

    // 3. Create a unified root structure node (Document)
    let root_document_node_id = document.add_object(dictionary! {
        "Type" => "StructElem",
        "S" => "Document",
        "K" => global_kids.clone(),
    });
    tracing::debug!(target: "pdf_merge", "Created root StructElem 'Document' (ID: {:?}) with {} top-level kids", root_document_node_id, global_kids.len());

    // 4. PARENT WIRING (/P): This is the "holy grail" of tag visibility in PDFix
    let mut reparented_count = 0;
    for child_ref in &global_kids {
        if let Ok(child_id) = child_ref.as_reference() {
            if let Ok(Object::Dictionary(dict)) = document.get_object_mut(child_id) {
                dict.set("P", root_document_node_id);
                reparented_count += 1;
            }
        }
    }
    tracing::debug!(target: "pdf_merge", "Successfully reparented {} structural elements to the new root", reparented_count);

    // 5. Create the final StructTreeRoot
    let struct_tree_root_id = document.add_object(dictionary! {
        "Type" => "StructTreeRoot",
        "K" => root_document_node_id,
        "ParentTree" => parent_tree_id,
        "ParentTreeNextKey" => final_offset as i32,
        "RoleMap" => global_role_map,
    });
    tracing::info!(target: "pdf_merge", "Final StructTreeRoot created (ID: {:?})", struct_tree_root_id);

    // 6. Update Catalog: link the structure and set the Marked flag
    if let Some(Object::Dictionary(cat_dict)) = document.objects.get_mut(&catalog_id) {
        cat_dict.set("Pages", pages_id);
        cat_dict.set("StructTreeRoot", struct_tree_root_id);
        cat_dict.set("MarkInfo", dictionary! { "Marked" => true });
        tracing::debug!(target: "pdf_merge", "Updated Catalog with StructTreeRoot and Marked flag");
    }

    // 7. Update Pages: set Count and Kids
    if let Some(Object::Dictionary(pag_dict)) = document.objects.get_mut(&pages_id) {
        pag_dict.set("Count", documents_pages.len() as u32);
        let kids_refs: Vec<Object> = documents_pages
            .keys()
            .copied()
            .map(Object::Reference)
            .collect();
        pag_dict.set("Kids", kids_refs);
        tracing::debug!(target: "pdf_merge", "Updated Pages root with {} page references", documents_pages.len());
    }

    Ok(document)
}
