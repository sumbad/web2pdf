use std::collections::VecDeque;

use lopdf::{Dictionary, Document, Object, ObjectId};

use super::helpers::*;

pub fn sanitize_pdf_ua(doc: &mut Document) -> anyhow::Result<()> {
    let Some(root_id) = find_struct_tree_root(doc) else {
        tracing::debug!("🛠️ Struct Tree not found!");
        return Ok(());
    };

    tracing::debug!("🛠️ Sanitize pdf");

    traverse_struct_tree(doc, root_id, |doc, id, dict| {
        flatten_nonstruct(doc, id, dict)?;
        sanitize_link_k(doc, id, dict)?;
        Ok(())
    })?;

    Ok(())
}

/// /// Public helper: принимает PDF bytes, делает flatten nonstruct pass и возвращает новые байты.
/// pub fn sanitize_pdf_ua(doc: &mut Document) -> anyhow::Result<()> {
///     if let Some(root_id) = find_struct_tree_root(doc) {
///         tracing::debug!("🛠️ Sanitize pdf");
///         // запускаем проход
///         flatten_nonstruct_tree(doc, root_id)?;
///     } else {
///         tracing::debug!("🛠️ Struct Tree not found!");
///         // не tagged — ничего не делаем
///     }
///
///     Ok(())
/// }

fn traverse_struct_tree<F>(
    doc: &mut Document,
    root_id: ObjectId,
    mut visit: F,
) -> anyhow::Result<()>
where
    F: FnMut(&mut Document, ObjectId, &Dictionary) -> anyhow::Result<()>,
{
    use std::collections::VecDeque;

    let mut queue = VecDeque::new();
    queue.push_back(root_id);

    while let Some(node_id) = queue.pop_front() {
        let obj = match doc.get_object(node_id) {
            Ok(o) => o.clone(),
            Err(_) => continue,
        };

        let dict = match obj.as_dict() {
            Ok(d) => d.clone(),
            Err(_) => continue,
        };

        // 🔹 пользовательская операция
        visit(doc, node_id, &dict)?;

        // 🔹 стандартный обход детей
        if let Ok(k) = dict.get(b"K") {
            match k {
                Object::Array(arr) => {
                    for item in arr {
                        if let Ok(id) = item.as_reference() {
                            queue.push_back(id);
                        }
                    }
                }
                Object::Reference(id) => {
                    queue.push_back(*id);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

pub fn sanitize_link_k(
    doc: &mut Document,
    node_id: ObjectId,
    dict: &Dictionary,
) -> anyhow::Result<()> {
    if !is_link_struct(dict) {
        return Ok(());
    }

    // let role = dict.get(b"S").and_then(|o| o.as_name()).ok();
    //
    // if role != Some(b"Link") {
    //     return Ok(());
    // }

    let k = match dict.get(b"K") {
        Ok(k) => k.clone(),
        Err(_) => return Ok(()),
    };

    let items = k.as_array()?;

    // let items: Vec<Object> = match k {
    //     Object::Array(arr) => arr,
    //     other => vec![other],
    // };

    let has_mcid = link_has_textual_content(doc, dict).unwrap_or(false);
    let mut has_objr = false;
    let mut new_k = Vec::new();

    tracing::debug!("Link has text {:?}", has_mcid);

    for item in items.iter() {
        tracing::debug!("Link item {:?}", item);
        // if is_mcid(item) {
        //     has_mcid = true;
        //     new_k.push(item.clone());
        // };

        if is_objr(item) {
            has_objr = true;
        }

        // if matches!(item, Object::Integer(_)) {
        //     has_mcid = true;
        //     new_k.push(item.clone());
        // } else if is_objr(item) {
        //     has_objr = true;
        // } else {
        //     new_k.push(item.clone());
        // }
    }

    if has_mcid && has_objr {
        tracing::debug!("🔧 Link {:?}: removed OBJR", node_id);
        let mut new_dict = dict.clone();
        new_dict.set("K", Object::Array(new_k));
        doc.objects.insert(node_id, Object::Dictionary(new_dict));
    }

    if has_objr && !has_mcid {
        tracing::warn!("⚠️ Link {:?}: OBJR without MCID", node_id);

        // // ищем MCID у родителя
        // if let Some(parent_id) = dict.get(b"P").ok().and_then(|o| o.as_reference().ok()) {
        //     tracing::debug!("Parent has P {:?}", parent_id);
        //     if let Some(mcid) = find_adjacent_mcid_in_parent(doc, parent_id, node_id) {
        //         attach_mcid_to_link(doc, node_id, mcid)?;
        //     } else {
        //         tracing::warn!(
        //             "⚠ Link {:?}: OBJR without MCID (no adjacent text found)",
        //             node_id
        //         );
        //     }
        // }
    }

    Ok(())
}

fn attach_mcid_to_link(
    doc: &mut Document,
    link_id: ObjectId,
    mcid_ref: Object,
) -> anyhow::Result<()> {
    let obj = doc.get_object(link_id)?.clone();
    let mut dict = obj.as_dict()?.clone();

    let mut new_k = match dict.get(b"K") {
        Ok(Object::Array(arr)) => arr.clone(),
        Ok(k) => vec![k.clone()],
        Err(_) => vec![],
    };

    // MCID должен быть первым
    new_k.insert(0, mcid_ref);

    dict.set("K", Object::Array(new_k));
    doc.objects.insert(link_id, Object::Dictionary(dict));

    tracing::debug!("🔗 Attached MCID to Link {:?}", link_id);

    Ok(())
}

fn find_adjacent_mcid_in_parent(
    doc: &Document,
    parent_id: ObjectId,
    link_id: ObjectId,
) -> Option<Object> {
    let parent = doc.get_object(parent_id).ok()?.clone();
    let dict = parent.as_dict().ok()?;
    let k = dict.get(b"K").ok()?;

    let arr = k.as_array().ok()?;

    tracing::debug!("Arr {:?}", arr);

    for (idx, item) in arr.iter().enumerate() {
        if let Object::Reference(id) = item
            && *id == link_id
        {
            tracing::debug!("Has as_reference {:?}", parent_id);
            // пробуем следующий
            if let Some(next) = arr.get(idx + 1) {
                if is_mcid(next) {
                    return Some(next.clone());
                }
            }
            // пробуем предыдущий
            if idx > 0 {
                if let Some(prev) = arr.get(idx - 1) {
                    if is_mcid(prev) {
                        return Some(prev.clone());
                    }
                }
            }
        }
    }

    None
}

pub fn flatten_nonstruct(
    doc: &mut Document,
    node_id: ObjectId,
    dict: &Dictionary,
) -> anyhow::Result<()> {
    let role = dict.get(b"S").and_then(|o| o.as_name()).ok();

    if role == Some(b"NonStruct") {
        return Ok(());
    }

    let k = match dict.get(b"K") {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };

    let arr = match k.as_array() {
        Ok(a) => a,
        Err(_) => return Ok(()),
    };

    let mut new_k = Vec::new();
    let mut changed = false;

    for item in arr {
        if let Ok(child_id) = item.as_reference()
            && let Ok(child_obj) = doc.get_object(child_id)
            && let Ok(child_dict) = child_obj.as_dict()
            && child_dict.get(b"S").and_then(|o| o.as_name()).ok() == Some(b"NonStruct")
            && let Ok(child_k) = child_dict.get(b"K")
        {
            changed = true;
            if let Ok(child_arr) = child_k.as_array() {
                new_k.extend(child_arr.clone());
            } else {
                new_k.push(child_k.clone());
            }
        } else {
            new_k.push(item.clone());
        }
    }

    if changed {
        let mut new_dict = dict.clone();
        new_dict.set("K", Object::Array(new_k));
        doc.objects.insert(node_id, Object::Dictionary(new_dict));
    }

    Ok(())
}

// /// Основной проход: рекурсивно обходит дерево и сворачивает NonStruct
// fn flatten_nonstruct_tree(doc: &mut Document, root_id: ObjectId) -> anyhow::Result<()> {
//     // Проходим в ширину, чтобы сначала обновить верхние узлы и потом детей
//     let mut q: VecDeque<ObjectId> = VecDeque::new();
//     q.push_back(root_id);
//
//     while let Some(node_id) = q.pop_front() {
//         // Получаем dict для узла
//         let obj = match doc.get_object(node_id) {
//             Ok(o) => o.clone(),
//             Err(_) => continue,
//         };
//         let dict = match obj.as_dict() {
//             Ok(d) => d.clone(),
//             Err(_) => continue,
//         };
//
//         // Берём роль S; это поле может отсутствовать (обычно StructElem)
//         let role_name = dict
//             .get(b"S")
//             .ok()
//             .and_then(|o| o.as_name().ok())
//             .map(|v| v.to_vec());
//
//         // Если это NonStruct — пропускаем (мы не "раскручиваем" NonStruct сам по себе)
//         if role_name.as_deref() == Some(b"NonStruct") {
//             // Но добавим его детей в очередь для дальнейшей проверки — теоретически там могут быть StructElem
//             if let Ok(k) = dict.get(b"K") {
//                 for kid in k.as_array().unwrap_or(&Vec::new()) {
//                     if let Object::Reference(id) = kid {
//                         q.push_back(*id);
//                     }
//                 }
//             }
//             continue;
//         }
//
//         // Рассматриваем K: если K — массив длиной 1, и единственный элемент — ссылка на StructElem,
//         // и этот child имеет S == NonStruct, то заменяем parent's K на child's K (effectively "unwrap")
//         if let Ok(k_obj) = dict.get(b"K") {
//             tracing::debug!("🛠️ K_obj {:?} ", k_obj);
//
//             if let Ok(k_arr) = k_obj.as_array() {
//                 let mut new_k: Vec<Object> = Vec::new();
//                 let mut changed = false;
//
//                 for item in k_arr {
//                     match item {
//                         Object::Reference(child_ref) => {
//                             if let Ok(child_obj) = doc.get_object(*child_ref)
//                                 && let Ok(child_dict) = child_obj.as_dict()
//                                 && let Some(child_role) =
//                                     child_dict.get(b"S").ok().and_then(|o| o.as_name().ok())
//                                 && child_role == b"NonStruct"
//                                 && let Some(child_k) = child_dict.get(b"K").ok()
//                             {
//                                 tracing::debug!("🛠️ Flattening NonStruct {:?}", child_ref);
//                                 // Вставляем содержимое NonStruct вместо него самого
//                                 if let Ok(child_k_arr) = child_k.as_array() {
//                                     new_k.extend(child_k_arr.clone());
//                                 } else {
//                                     new_k.push(child_k.clone());
//                                 }
//                                 changed = true;
//                             } else {
//                                 new_k.push(item.clone());
//                             }
//                         }
//                         _ => {
//                             new_k.push(item.clone());
//                         }
//                     }
//                 }
//
//                 if changed {
//                     let mut new_parent = dict.clone();
//                     new_parent.set("K", Object::Array(new_k.clone()));
//                     doc.objects.insert(node_id, Object::Dictionary(new_parent));
//                 }
//
//                 // Рекурсивный обход
//                 for kid in &new_k {
//                     if let Object::Reference(id) = kid {
//                         q.push_back(*id);
//                     }
//                 }
//
//                 // if k_arr.len() == 1
//                 //     && let Some(first) = k_arr.first()
//                 //     && let Ok(child_ref) = first.as_reference()
//                 //     && let Ok(child_obj) = doc.get_object(child_ref)
//                 //     && let Ok(child_dict) = child_obj.as_dict()
//                 //     && let Some(child_role) =
//                 //         child_dict.get(b"S").ok().and_then(|o| o.as_name().ok())
//                 // {
//                 //     tracing::debug!("🛠️ Child role {:?} ", child_role);
//                 //     if child_role == b"NonStruct"
//                 //         && let Ok(child_k) = child_dict.get(b"K")
//                 //     {
//                 //         let child_k_clone = child_k.clone();
//                 //         let mut new_parent = dict.clone();
//                 //         new_parent.set("K", child_k_clone.clone());
//                 //         doc.objects.insert(node_id, Object::Dictionary(new_parent));
//                 //
//                 //         if let Ok(new_k_arr) = child_k_clone.as_array() {
//                 //             for kid in new_k_arr {
//                 //                 if let Object::Reference(id) = kid {
//                 //                     q.push_back(*id);
//                 //                 }
//                 //             }
//                 //         }
//                 //         continue;
//                 //     }
//                 // } else {
//                 //     // если больше одного ребёнка — добавить их в очередь, чтобы обработать рекурсивно
//                 //     for kid in k_arr {
//                 //         if let Object::Reference(id) = kid {
//                 //             q.push_back(*id);
//                 //         }
//                 //     }
//                 // }
//             } else if let Ok(single_ref) = k_obj.as_reference() {
//                 // аналогично, если K - single reference (не массив)
//                 q.push_back(single_ref);
//             }
//         }
//     }
//
//     Ok(())
// }

// pub fn flatten_nonstruct(doc: &mut Document, obj_id: lopdf::ObjectId) {
//     let obj = doc.get_object(obj_id).unwrap();
//
//     let dict = match obj.as_dict() {
//         Ok(d) => d.clone(),
//         Err(_) => return,
//     };
//
//     let s = dict.get(b"S").and_then(|o| o.as_name().ok());
//
//     if let Some(k) = dict.get(b"K") {
//         // Интересует только массив из одного элемента
//         if let Ok(kids) = k.as_array() {
//             if kids.len() == 1 {
//                 if let Ok((child_id, _)) = kids[0].as_reference() {
//                     if let Ok(child) = doc.get_object(child_id) {
//                         if let Ok(child_dict) = child.as_dict() {
//                             let child_s = child_dict.get(b"S").and_then(|o| o.as_name().ok());
//
//                             if child_s == Some(b"NonStruct") {
//                                 if let Some(child_k) = child_dict.get(b"K") {
//                                     // Переносим K наверх
//                                     let mut new_dict = dict.clone();
//                                     new_dict.set("K", child_k.clone());
//
//                                     doc.objects.insert(obj_id, Object::Dictionary(new_dict));
//                                 }
//                             }
//                         }
//                     }
//                 }
//             }
//         }
//     }
// }
