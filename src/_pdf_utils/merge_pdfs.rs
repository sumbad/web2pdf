use lopdf::{Bookmark, Dictionary, Document, Object, ObjectId, dictionary};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use super::fix_tagged_pdf::fix_tagged_pdf;
use super::flatten_nonstruct::sanitize_pdf_ua;
use crate::toc::TocNode;

/// Результат обработки структуры одного документа
pub struct DocStructureData {
    /// Смещенные элементы ParentTree (ключ-значение для массива Nums)
    pub shifted_nums: Vec<Object>,
    /// Массив детей структуры (уже "сплющенный")
    pub root_kids: Vec<Object>,
    /// Словарь соответствия кастомных тегов (RoleMap)
    pub role_map: Option<Dictionary>,
    /// На сколько нужно сдвинуть офсет для следующего документа
    pub next_offset_increment: i64,
}

pub fn extract_and_shift_structure(doc: &mut Document, current_offset: i64) -> DocStructureData {
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
                // --- А. Получаем ParentTreeNextKey для расчета будущего смещения ---
                local_next_key = str_root
                    .get(b"ParentTreeNextKey")
                    .and_then(|o| o.as_i64())
                    .unwrap_or(0);

                // --- Б. Сдвигаем ключи в ParentTree (Nums) ---
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

                // --- В. Извлекаем и сплющиваем детей структуры (K) ---
                if let Ok(k_obj) = str_root.get(b"K") {
                    match k_obj {
                        Object::Array(arr) => {
                            root_kids.extend(arr.iter().cloned());
                        }
                        Object::Reference(id) => {
                            // Проверяем: не является ли этот объект узлом типа "Document"
                            let is_doc_node = doc
                                .get_object(*id)
                                .ok()
                                .and_then(|o| o.as_dict().ok())
                                .and_then(|d| d.get(b"S").ok())
                                .and_then(|s| s.as_name().ok())
                                == Some(b"Document");

                            if is_doc_node {
                                // Если это Document, берем его детей (/K) напрямую
                                if let Ok(inner_k) =
                                    doc.get_object(*id).and_then(|o| o.as_dict()?.get(b"K"))
                                {
                                    match inner_k {
                                        Object::Array(arr) => root_kids.extend(arr.iter().cloned()),
                                        _ => root_kids.push(inner_k.clone()),
                                    }
                                }
                            } else {
                                // Если это не Document (например, Div или Part), просто добавляем ссылку
                                root_kids.push(k_obj.clone());
                            }
                        }
                        _ => root_kids.push(k_obj.clone()),
                    }
                }

                // --- Г. Извлекаем RoleMap ---
                role_map = str_root
                    .get(b"RoleMap")
                    .ok()
                    .and_then(|o| o.as_dict().ok())
                    .map(|d| d.clone());
            }
        }
    }

    // --- Д. Сдвигаем StructParents на страницах (самое важное для связи) ---
    for (_page_num, page_id) in doc.get_pages() {
        if let Ok(page_dict) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            if let Ok(old_sp) = page_dict.get(b"StructParents").and_then(|o| o.as_i64()) {
                page_dict.set("StructParents", old_sp + current_offset);
            }
        }
    }

    // Рассчитываем инкремент: сколько индексов занял этот документ.
    // Берем максимум между NextKey и реальным количеством страниц.
    let page_count = doc.get_pages().len() as i64;
    let increment = local_next_key.max(page_count).max(1);

    DocStructureData {
        shifted_nums,
        root_kids,
        role_map,
        next_offset_increment: increment,
    }
}

///
///
///
///
///
///
pub fn assemble_merged_document(
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

    // 1. Вставляем страницы в итоговый документ и связываем их с новым Pages ID
    for (id, obj) in &documents_pages {
        if let Ok(dict) = obj.as_dict() {
            let mut dict = dict.clone();
            dict.set("Parent", pages_id);
            document.objects.insert(*id, Object::Dictionary(dict));
        }
    }
    tracing::debug!(target: "pdf_merge", "Linked {} pages to the new Pages root (ID: {:?})", documents_pages.len(), pages_id);

    // 2. Создаем единый объект ParentTree (Nums)
    let parent_tree_id = document.add_object(dictionary! {
        "Nums" => global_nums.clone(),
    });
    tracing::debug!(target: "pdf_merge", "Created ParentTree (ID: {:?}) with {} entries", parent_tree_id, global_nums.len() / 2);

    // 3. Создаем единый корневой узел структуры (Document)
    let root_document_node_id = document.add_object(dictionary! {
        "Type" => "StructElem",
        "S" => "Document",
        "K" => global_kids.clone(),
    });
    tracing::debug!(target: "pdf_merge", "Created root StructElem 'Document' (ID: {:?}) with {} top-level kids", root_document_node_id, global_kids.len());

    // 4. ПРОШИВКА РОДИТЕЛЕЙ (/P): Это "святой грааль" видимости тегов в PDFix
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

    // 5. Создаем финальный StructTreeRoot
    let struct_tree_root_id = document.add_object(dictionary! {
        "Type" => "StructTreeRoot",
        "K" => root_document_node_id,
        "ParentTree" => parent_tree_id,
        "ParentTreeNextKey" => final_offset as i32,
        "RoleMap" => global_role_map,
    });
    tracing::info!(target: "pdf_merge", "Final StructTreeRoot created (ID: {:?})", struct_tree_root_id);

    // 6. Обновляем Catalog: привязываем структуру и ставим флаг Marked
    if let Some(Object::Dictionary(cat_dict)) = document.objects.get_mut(&catalog_id) {
        cat_dict.set("Pages", pages_id);
        cat_dict.set("StructTreeRoot", struct_tree_root_id);
        cat_dict.set("MarkInfo", dictionary! { "Marked" => true });
        tracing::debug!(target: "pdf_merge", "Updated Catalog with StructTreeRoot and Marked flag");
    }

    // 7. Обновляем Pages: устанавливаем Count и Kids
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

///
///
///
///
///
///
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

        sanitize_pdf_ua(&mut doc);

        // 📌 Ренумерация
        let start_id = max_id;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        tracing::debug!(
            target: "pdf_merge",
            "Renumbered objects for '{}': IDs shifted from {} to {}",
            title, start_id, doc.max_id
        );

        // 📌 Шаг 1.3: Экстракция данных StructTreeRoot
        // Находим корень структуры в текущем документе
        let mut struct_found = false;
        if let Ok(catalog) = doc.catalog() {
            if let Ok(struct_root_res) = catalog.get(b"StructTreeRoot") {
                // Сохраняем ссылку на StructTreeRoot этого документа для этапов 2-4
                if let Ok(id) = struct_root_res.as_reference() {
                    if let Ok(dict) = doc.get_object(id).and_then(|o| o.as_dict()) {
                        // Клонируем словарь, так как doc будет поглощен или уничтожен
                        source_struct_roots.push(dict.clone());

                        struct_found = true;

                        // Логируем ключи, которые есть в структуре (K, ParentTree, RoleMap и т.д.)
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

        // --- Вызов функции обработки структуры ---
        let struct_data = extract_and_shift_structure(&mut doc, current_offset);

        // 1. Собираем Nums (ParentTree)
        global_nums.extend(struct_data.shifted_nums);

        // 2. Собираем детей (K) - теперь просто extend, без if let Some
        global_kids.extend(struct_data.root_kids);

        // 3. Собираем RoleMap
        if let Some(rm) = struct_data.role_map {
            for (k, v) in rm {
                global_role_map.set(k.clone(), v.clone());
            }
        }

        // 4. Обновляем глобальный офсет для следующего файла
        current_offset += struct_data.next_offset_increment;

        tracing::debug!(
            target: "pdf_merge",
            "Processed structure for '{}': Shifted {} Nums, incremented offset by {}",
            title, global_nums.len() / 2, struct_data.next_offset_increment
        );

        // 📑 Сбор страниц и объектов
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

            // Важно: сохраняем страницу
            if let Ok(obj) = doc.get_object(object_id) {
                documents_pages.insert(object_id, obj.to_owned());
                file_page_count += 1;
            }
        }

        tracing::debug!(target: "pdf_merge", "Collected {} pages from '{}'. Current total pagenum: {}", file_page_count, title, pagenum + file_page_count - 1);

        // Поглощаем все объекты текущего документа
        documents_objects.extend(doc.objects);
    }

    tracing::info!(
        target: "pdf_merge",
        "Stage 1 complete: Total objects: {}, Total pages: {}, Struct roots collected: {}",
        documents_objects.len(), documents_pages.len(), source_struct_roots.len()
    );

    ////////////////////////////////////////////////////////////////////////////////////////////////////
    // --- ЭТАП 4: Определение базовых ID и синхронизация ---

    // ⚠️ КРИТИЧНО: Синхронизируем счетчик ID в новом документе с тем, что мы насчитали в цикле
    document.max_id = max_id;

    let mut catalog_id: Option<ObjectId> = None;
    let mut pages_id: Option<ObjectId> = None;

    // Сначала просто переносим все общие объекты (шрифты, ресурсы)
    for (id, obj) in &documents_objects {
        match obj.type_name().unwrap_or(b"") {
            b"Catalog" => {
                if catalog_id.is_none() {
                    catalog_id = Some(*id);
                    // ⚠️ ОБЯЗАТЕЛЬНО: вставляем в документ, чтобы assemble_merged_document мог его найти через get_mut
                    document.objects.insert(*id, obj.clone());
                }
            }
            b"Pages" => {
                if pages_id.is_none() {
                    pages_id = Some(*id);
                    // ⚠️ ОБЯЗАТЕЛЬНО: вставляем в документ
                    document.objects.insert(*id, obj.clone());
                }
            }
            b"Page" | b"Outlines" | b"Outline" | b"StructTreeRoot" => {
                // Эти типы мы пересобираем вручную, пропускаем
            }
            _ => {
                document.objects.insert(*id, obj.clone());
            }
        }
    }

    let catalog_id = catalog_id.expect("Catalog not found");
    let pages_id = pages_id.expect("Pages root not found");

    // --- ЭТАП 5: Финальная сборка ---
    // Теперь assemble_merged_document получит ID, начинающиеся с max_id + 1 (т.е. с 370+)
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

    // --- ФИНАЛИЗАЦИЯ ---
    document.trailer = dictionary! {
        "Root" => catalog_id,
        "Size" => (document.objects.len() as i64) + 1
    };

    // ⚠️ Сдвигаем max_id на актуальное значение после добавления новых объектов структуры
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

    // ⚠️ ОЧЕНЬ РЕКОМЕНДУЕТСЯ: перенумеровать все объекты в самом конце для "чистоты" xref-таблицы
    document.renumber_objects(); 

    document.compress();
    document.save(output)?;

    tracing::info!("Merged PDF saved successfully.");
    Ok(())

    // // "Catalog" and "Pages" are mandatory.
    // let mut catalog_object: Option<(ObjectId, Object)> = None;
    // let mut pages_object: Option<(ObjectId, Object)> = None;
    //
    // // Process all objects except "Page" type
    // for (object_id, object) in documents_objects.iter() {
    //     // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects.
    //     // All other objects should be collected and inserted into the main Document.
    //     match object.type_name().unwrap_or(b"") {
    //         b"Catalog" => {
    //             // Collect a first "Catalog" object and use it for the future "Pages".
    //             catalog_object = Some((
    //                 if let Some((id, _)) = catalog_object {
    //                     id
    //                 } else {
    //                     *object_id
    //                 },
    //                 object.clone(),
    //             ));
    //         }
    //         b"Pages" => {
    //             // Collect and update a first "Pages" object and use it for the future "Catalog"
    //             // We have also to merge all dictionaries of the old and the new "Pages" object
    //             if let Ok(dictionary) = object.as_dict() {
    //                 let mut dictionary = dictionary.clone();
    //                 if let Some((_, ref object)) = pages_object
    //                     && let Ok(old_dictionary) = object.as_dict()
    //                 {
    //                     dictionary.extend(old_dictionary);
    //                 }
    //
    //                 pages_object = Some((
    //                     if let Some((id, _)) = pages_object {
    //                         id
    //                     } else {
    //                         *object_id
    //                     },
    //                     Object::Dictionary(dictionary),
    //                 ));
    //             }
    //         }
    //         b"Page" => {}     // Ignored, processed later and separately
    //         b"Outlines" => {} // Ignored, not supported yet
    //         b"Outline" => {}  // Ignored, not supported yet
    //         _ => {
    //             document.objects.insert(*object_id, object.clone());
    //         }
    //     }
    // }
    //
    // // If no "Pages" object found, return early (no PDFs to merge).
    // if pages_object.is_none() {
    //     println!("  ⚠️ No pages found to merge");
    //     return Ok(());
    // }
    //
    // // Iterate over all "Page" objects and collect into the parent "Pages" created before
    // for (object_id, object) in documents_pages.iter() {
    //     if let Ok(dictionary) = object.as_dict() {
    //         let mut dictionary = dictionary.clone();
    //         dictionary.set("Parent", pages_object.as_ref().unwrap().0);
    //
    //         document
    //             .objects
    //             .insert(*object_id, Object::Dictionary(dictionary));
    //     }
    // }
    //
    // // If no "Catalog" found, abort.
    // if catalog_object.is_none() {
    //     println!("Catalog root not found.");
    //
    //     return Ok(());
    // }
    //
    // let catalog_object = catalog_object.unwrap();
    // let pages_object = pages_object.unwrap();
    //
    // // Build a new "Pages" with updated fields
    // if let Ok(dictionary) = pages_object.1.as_dict() {
    //     let mut dictionary = dictionary.clone();
    //
    //     // Set new pages count
    //     dictionary.set("Count", documents_pages.len() as u32);
    //
    //     // Set new "Kids" list (collected from documents pages) for "Pages"
    //     let page_ids: Vec<_> = documents_pages.keys().copied().collect();
    //     dictionary.set(
    //         "Kids",
    //         page_ids
    //             .into_iter()
    //             .map(Object::Reference)
    //             .collect::<Vec<_>>(),
    //     );
    //
    //     document
    //         .objects
    //         .insert(pages_object.0, Object::Dictionary(dictionary));
    // }
    //
    // // Insert catalog object and link it to pages
    // if let Ok(dictionary) = catalog_object.1.as_dict() {
    //     let mut dictionary = dictionary.clone();
    //     dictionary.set("Pages", pages_object.0);
    //     document
    //         .objects
    //         .insert(catalog_object.0, Object::Dictionary(dictionary));
    // }
    //
    // document.trailer = lopdf::Dictionary::new();
    // document.trailer.set("Root", catalog_object.0);
    // document
    //     .trailer
    //     .set("Size", (document.objects.len() as i64) + 1);
    //
    // // Update the max internal ID as wasn't updated before due to direct objects insertion
    // document.max_id = document.objects.len() as u32;
    //
    // // Reorder all new Document objects
    // document.renumber_objects();
    //
    // // Set any Bookmarks to the First child if they are not set to a page
    // document.adjust_zero_pages();
    //
    // // Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
    // println!(
    //     "  🔗 Building outline from {} bookmarks",
    //     document.bookmarks.len()
    // );
    //
    // if document.bookmarks.is_empty() {
    //     println!("  ⚠️ No bookmarks to create outline");
    // } else {
    //     println!("  📚 Bookmarks found: {:?}", document.bookmarks);
    //
    //     match document.build_outline() {
    //         Some(outline_id) => {
    //             println!("  ✅ Outline created with ID: {:?}", outline_id);
    //
    //             // Get the actual catalog ID from the trailer after renumbering
    //             let catalog_id = document
    //                 .trailer
    //                 .get(b"Root")
    //                 .and_then(|root| root.as_reference())
    //                 .unwrap_or(catalog_object.0);
    //             println!("  📄 Catalog ID: {:?}", catalog_id);
    //
    //             // Ensure the outline object has proper structure
    //             if let Ok(outline_obj) = document.get_object(outline_id)
    //                 && let Object::Dictionary(mut outline_dict) = outline_obj.clone()
    //             {
    //                 // Add Count property (number of bookmarks)
    //                 outline_dict.set("Count", document.bookmarks.len() as i64);
    //
    //                 // Update the outline object
    //                 if let Ok(obj) = document.get_object_mut(outline_id) {
    //                     *obj = Object::Dictionary(outline_dict);
    //                     println!(
    //                         "  ✅ Enhanced outline with Count: {}",
    //                         document.bookmarks.len()
    //                     );
    //                 }
    //             }
    //
    //             match document.get_object_mut(catalog_id) {
    //                 Ok(Object::Dictionary(dict)) => {
    //                     dict.set("Outlines", Object::Reference(outline_id));
    //                     println!("  ✅ Outline added to catalog");
    //                 }
    //                 Ok(Object::Stream(stream)) => {
    //                     // Handle linearized PDFs - convert to dictionary
    //                     let mut new_dict = stream.dict.clone();
    //                     new_dict.set("Outlines", Object::Reference(outline_id));
    //                     *document.get_object_mut(catalog_id).unwrap() =
    //                         Object::Dictionary(new_dict);
    //                     println!("  ✅ Outline added to linearized catalog");
    //                 }
    //                 Ok(other) => {
    //                     println!("  ❌ Catalog object type: {:?}", other.type_name());
    //                     // Try to force it to be a dictionary
    //                     if let Err(e) = document.get_object_mut(catalog_id).map(|obj| {
    //                         *obj = Object::Dictionary(lopdf::Dictionary::new());
    //                     }) {
    //                         println!("  ❌ Failed to convert catalog to dictionary: {}", e);
    //                     }
    //                 }
    //                 Err(e) => {
    //                     println!("  ❌ Failed to get catalog object: {}", e);
    //                 }
    //             }
    //         }
    //         None => {
    //             println!("  ❌ Failed to build outline");
    //         }
    //     }
    // }
    //
    // fix_tagged_pdf(&mut document)?;
    //
    // // Check if StructTreeRoot exists in catalog
    // if let Ok(catalog_dict) = document.catalog() {
    //     if let Ok(root) = catalog_dict.get(b"StructTreeRoot") {
    //         println!("✅ StructTreeRoot found: {:?}", root);
    //     } else {
    //         println!("❌ StructTreeRoot missing");
    //     }
    // } else {
    //     println!("⚠️ Failed to get catalog");
    // }
    //
    // // Check trailer for Marked flag
    // if let Ok(marked) = document.trailer.get(b"Marked") {
    //     println!("✅ Trailer Marked: {:?}", marked);
    // } else {
    //     println!("❌ Trailer does not contain Marked key");
    // }
    //
    // document.compress();
    //
    // document.save(output)?;
    //
    // println!("{:#?}", document.trailer);
    //
    // Ok(())
}
