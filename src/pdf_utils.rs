use lopdf::{Bookmark, Dictionary, Document, Object, ObjectId};
use std::{collections::BTreeMap, path::Path};

pub fn merge_pdfs<P>(files_with_titles: Vec<(P, String)>, output: P) -> lopdf::Result<()>
where
    P: AsRef<Path>,
{
    let files_iter = files_with_titles.into_iter();

    // Define a starting `max_id` (will be used as start index for object_ids).
    let mut max_id = 1;
    let mut pagenum = 1;
    // Collect all Documents Objects grouped by a map
    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for (path, title) in files_iter {
        let path_ref = path.as_ref();

        // ⚠️ Skip corrupted PDFs
        let mut doc = match Document::load(path_ref) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("⚠️ Skipping corrupted PDF {:?}: {:?}", path_ref, e);
                continue;
            }
        };

        // 📌 Shift IDs to prevent intersections between documents
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        // 📑 Add pages with bookmarks
        let mut is_first_page = true;
        documents_pages.extend(
            IntoIterator::into_iter(doc.get_pages())
                .map(|(_page_num, object_id)| {
                    // Create bookmark for the first page of this document
                    if is_first_page {
                        println!("  🔖 Creating bookmark: {}", title);
                        let bookmark =
                            Bookmark::new(title.clone(), [0.0, 0.0, 1.0], pagenum - 1, object_id);
                        document.add_bookmark(bookmark, None);
                        is_first_page = false;
                    }
                    pagenum += 1;

                    (object_id, doc.get_object(object_id).unwrap().to_owned())
                })
                .collect::<BTreeMap<ObjectId, Object>>(),
        );
        documents_objects.extend(doc.objects);
    }

    // "Catalog" and "Pages" are mandatory.
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    // Process all objects except "Page" type
    for (object_id, object) in documents_objects.iter() {
        // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects.
        // All other objects should be collected and inserted into the main Document.
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                // Collect a first "Catalog" object and use it for the future "Pages".
                catalog_object = Some((
                    if let Some((id, _)) = catalog_object {
                        id
                    } else {
                        *object_id
                    },
                    object.clone(),
                ));
            }
            b"Pages" => {
                // Collect and update a first "Pages" object and use it for the future "Catalog"
                // We have also to merge all dictionaries of the old and the new "Pages" object
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref object)) = pages_object
                        && let Ok(old_dictionary) = object.as_dict()
                    {
                        dictionary.extend(old_dictionary);
                    }

                    pages_object = Some((
                        if let Some((id, _)) = pages_object {
                            id
                        } else {
                            *object_id
                        },
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" => {}     // Ignored, processed later and separately
            b"Outlines" => {} // Ignored, not supported yet
            b"Outline" => {}  // Ignored, not supported yet
            _ => {
                document.objects.insert(*object_id, object.clone());
            }
        }
    }

    // If no "Pages" object found, return early (no PDFs to merge).
    if pages_object.is_none() {
        println!("  ⚠️ No pages found to merge");
        return Ok(());
    }

    // Iterate over all "Page" objects and collect into the parent "Pages" created before
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_object.as_ref().unwrap().0);

            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // If no "Catalog" found, abort.
    if catalog_object.is_none() {
        println!("Catalog root not found.");

        return Ok(());
    }

    let catalog_object = catalog_object.unwrap();
    let pages_object = pages_object.unwrap();

    // Build a new "Pages" with updated fields
    if let Ok(dictionary) = pages_object.1.as_dict() {
        let mut dictionary = dictionary.clone();

        // Set new pages count
        dictionary.set("Count", documents_pages.len() as u32);

        // Set new "Kids" list (collected from documents pages) for "Pages"
        let page_ids: Vec<_> = documents_pages.keys().copied().collect();
        dictionary.set(
            "Kids",
            page_ids
                .into_iter()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );

        document
            .objects
            .insert(pages_object.0, Object::Dictionary(dictionary));
    }

    // Insert catalog object and link it to pages
    if let Ok(dictionary) = catalog_object.1.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_object.0);
        document
            .objects
            .insert(catalog_object.0, Object::Dictionary(dictionary));
    }

    document.trailer = lopdf::Dictionary::new();
    document.trailer.set("Root", catalog_object.0);
    document
        .trailer
        .set("Size", (document.objects.len() as i64) + 1);

    // Update the max internal ID as wasn't updated before due to direct objects insertion
    document.max_id = document.objects.len() as u32;

    // Reorder all new Document objects
    document.renumber_objects();

    // Set any Bookmarks to the First child if they are not set to a page
    document.adjust_zero_pages();

    // Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
    println!(
        "  🔗 Building outline from {} bookmarks",
        document.bookmarks.len()
    );

    if document.bookmarks.is_empty() {
        println!("  ⚠️ No bookmarks to create outline");
    } else {
        println!("  📚 Bookmarks found: {:?}", document.bookmarks);

        match document.build_outline() {
            Some(outline_id) => {
                println!("  ✅ Outline created with ID: {:?}", outline_id);

                // Get the actual catalog ID from the trailer after renumbering
                let catalog_id = document
                    .trailer
                    .get(b"Root")
                    .and_then(|root| root.as_reference())
                    .unwrap_or(catalog_object.0);
                println!("  📄 Catalog ID: {:?}", catalog_id);

                // Ensure the outline object has proper structure
                if let Ok(outline_obj) = document.get_object(outline_id)
                    && let Object::Dictionary(mut outline_dict) = outline_obj.clone() {
                    // Add Count property (number of bookmarks)
                    outline_dict.set("Count", document.bookmarks.len() as i64);

                    // Update the outline object
                    if let Ok(obj) = document.get_object_mut(outline_id) {
                        *obj = Object::Dictionary(outline_dict);
                        println!(
                            "  ✅ Enhanced outline with Count: {}",
                            document.bookmarks.len()
                        );
                    }
                }

                match document.get_object_mut(catalog_id) {
                    Ok(Object::Dictionary(dict)) => {
                        dict.set("Outlines", Object::Reference(outline_id));
                        println!("  ✅ Outline added to catalog");
                    }
                    Ok(Object::Stream(stream)) => {
                        // Handle linearized PDFs - convert to dictionary
                        let mut new_dict = stream.dict.clone();
                        new_dict.set("Outlines", Object::Reference(outline_id));
                        *document.get_object_mut(catalog_id).unwrap() =
                            Object::Dictionary(new_dict);
                        println!("  ✅ Outline added to linearized catalog");
                    }
                    Ok(other) => {
                        println!("  ❌ Catalog object type: {:?}", other.type_name());
                        // Try to force it to be a dictionary
                        if let Err(e) = document.get_object_mut(catalog_id).map(|obj| {
                            *obj = Object::Dictionary(lopdf::Dictionary::new());
                        }) {
                            println!("  ❌ Failed to convert catalog to dictionary: {}", e);
                        }
                    }
                    Err(e) => {
                        println!("  ❌ Failed to get catalog object: {}", e);
                    }
                }
            }
            None => {
                println!("  ❌ Failed to build outline");
            }
        }
    }

    fix_tagged_pdf(&mut document)?;

    // Check if StructTreeRoot exists in catalog
    if let Ok(catalog_dict) = document.catalog() {
        if let Ok(root) = catalog_dict.get(b"StructTreeRoot") {
            println!("✅ StructTreeRoot found: {:?}", root);
        } else {
            println!("❌ StructTreeRoot missing");
        }
    } else {
        println!("⚠️ Failed to get catalog");
    }

    // Check trailer for Marked flag
    if let Ok(marked) = document.trailer.get(b"Marked") {
        println!("✅ Trailer Marked: {:?}", marked);
    } else {
        println!("❌ Trailer does not contain Marked key");
    }

    document.compress();

    document.save(output)?;

    println!("{:#?}", document.trailer);

    Ok(())
}

pub fn fix_tagged_pdf(doc: &mut Document) -> lopdf::Result<()> {
    let has_struct_tree = doc
        .catalog()
        .ok()
        .and_then(|c| c.get(b"StructTreeRoot").ok())
        .is_some();

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
