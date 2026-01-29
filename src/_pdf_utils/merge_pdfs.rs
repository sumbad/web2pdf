use lopdf::{Bookmark, Document, Object, ObjectId};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use super::fix_tagged_pdf::fix_tagged_pdf;
use super::flatten_nonstruct::sanitize_pdf_ua;
use crate::toc::TocNode;

pub fn merge_pdfs<P>(toc: Vec<TocNode>, output: P) -> lopdf::Result<()>
where
    P: AsRef<Path>,
{
    let toc_iter = toc.into_iter();

    // 📌 Шаг 1.1: Используем версию 1.7 для поддержки современного Tagged PDF
    let mut document = Document::with_version("1.7");

    let mut max_id = 1;
    let mut pagenum = 1;

    let mut documents_pages = BTreeMap::new();
    let mut documents_objects = BTreeMap::new();

    // 📌 Шаг 1.2: Коллекторы для структурных данных (Этап 1)
    // Мы сохраним StructTreeRoot каждого документа как отдельные объекты для последующего анализа
    let mut source_struct_roots = Vec::new();

    let mut previous_lever_bookmark: HashMap<u8, Option<u32>> = HashMap::new();

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

        let mut doc = match Document::load(file_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("⚠️ Skipping corrupted PDF {:?}: {:?}", file_path, e);
                continue;
            }
        };

        sanitize_pdf_ua(&mut doc);

        // 📌 Ренумерация (уже есть у вас, это правильно)
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        // 📌 Шаг 1.3: Экстракция данных StructTreeRoot
        // Находим корень структуры в текущем документе
        if let Ok(catalog) = doc.catalog() {
            if let Ok(struct_root_res) = catalog.get(b"StructTreeRoot") {
                // Сохраняем ссылку на StructTreeRoot этого документа для этапов 2-4
                if let Ok(id) = struct_root_res.as_reference() {
                    if let Ok(dict) = doc.get_object(id).and_then(|o| o.as_dict()) {
                        // Клонируем словарь, так как doc будет поглощен или уничтожен
                        source_struct_roots.push(dict.clone());
                    }
                }
            }
        }

        // 📑 Сбор страниц и объектов (ваш текущий код)
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

            // Важно: сохраняем страницу
            if let Ok(obj) = doc.get_object(object_id) {
                documents_pages.insert(object_id, obj.to_owned());
            }
        }

        // Поглощаем все объекты текущего документа
        documents_objects.extend(doc.objects);
    }
    /////////////////

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
                    && let Object::Dictionary(mut outline_dict) = outline_obj.clone()
                {
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
