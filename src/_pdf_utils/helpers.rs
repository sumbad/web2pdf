use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

pub fn find_struct_tree_root(doc: &Document) -> Option<ObjectId> {
    doc.catalog()
        .ok()
        .and_then(|cat| cat.get(b"StructTreeRoot").ok())
        .and_then(|obj| obj.as_reference().ok())
}

pub fn is_link_struct(dict: &Dictionary) -> bool {
    if let Ok(obj) = dict.get(b"S") {
        is_link_obj(obj)
    } else {
        false
    }
}

pub fn is_link_obj(obj: &Object) -> bool {
    obj.as_name().map(|n| n == b"Link").unwrap_or(false)
}

pub fn is_objr(obj: &Object) -> bool {
    match obj {
        Object::Dictionary(d) => d
            .get(b"Type")
            .and_then(|t| t.as_name())
            .map(|n| n == b"OBJR")
            .unwrap_or(false),
        _ => false,
    }
}

pub fn is_mcid(obj: &Object) -> bool {
    matches!(obj, Object::Integer(_))
}

pub fn link_has_textual_content(doc: &Document, dict: &Dictionary) -> anyhow::Result<bool> {
    // let elem = doc.get_object(elem_ref.as_reference()?)?;
    // let dict = elem.as_dict()?;

    let k = match dict.get(b"K") {
        Ok(k) => k,
        Err(_) => return Ok(false),
    };

    let items = match k {
        Object::Array(arr) => arr.clone(),
        other => vec![other.clone()],
    };

    for item in items {
        match item {
            item if is_mcid(&item) => return Ok(true),
            Object::Reference(..) => {
                let child = doc.get_object(item.as_reference()?)?;
                let child_dict = child.as_dict()?;

                // StructElem without its own /K → implicit MCID
                if child_dict.get(b"K").is_err() {
                    return Ok(true);
                }

                // recurse
                if link_has_textual_content(doc, child_dict)? {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }

    Ok(false)
}

pub fn get_kids_as_vec(dict: &Dictionary) -> anyhow::Result<Vec<Object>> {
    match dict.get(b"K") {
        Ok(Object::Array(arr)) => Ok(arr.clone()),
        Ok(obj) => Ok(vec![obj.clone()]),
        _ => Ok(vec![]),
    }
}

pub fn node_is_nonstruct(doc: &Document, id: ObjectId) -> bool {
    doc.get_object(id)
        .and_then(|o| o.as_dict())
        .map(|d| is_nonstruct(d))
        .unwrap_or(false)
}

pub fn is_nonstruct(dict: &Dictionary) -> bool {
    dict.get(b"S").and_then(|o| o.as_name()).ok() == Some(b"NonStruct")
}

pub fn set_parent_link(doc: &mut Document, node_id: ObjectId, parent_id: ObjectId) {
    if let Ok(Object::Dictionary(dict)) = doc.get_object_mut(node_id) {
        dict.set("P", Object::Reference(parent_id));
    }
}

pub fn collect_all_node_ids(
    doc: &Document,
    id: ObjectId,
    nodes: &mut Vec<ObjectId>,
    visited: &mut HashSet<ObjectId>,
) {
    if visited.contains(&id) {
        return;
    }
    visited.insert(id);
    nodes.push(id);

    if let Ok(dict) = doc.get_object(id).and_then(|o| o.as_dict()) {
        if let Ok(kids) = dict.get(b"K") {
            let kids_vec = match kids {
                Object::Array(arr) => arr.clone(),
                other => vec![other.clone()],
            };
            for kid in kids_vec {
                if let Ok(kid_id) = kid.as_reference() {
                    collect_all_node_ids(doc, kid_id, nodes, visited);
                }
            }
        }
    }
}

pub fn find_struct_tree_root_id(doc: &Document) -> Option<ObjectId> {
    doc.catalog()
        .and_then(|cat| cat.get(b"StructTreeRoot"))
        .and_then(|root| root.as_reference())
        .ok()
}

pub fn get_node_role(doc: &Document, id: ObjectId) -> Option<Vec<u8>> {
    doc.get_object(id)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"S").ok())
        .and_then(|s| s.as_name().ok())
        .map(|n| n.to_vec())
}

pub fn is_text_container(role: &[u8]) -> bool {
    matches!(
        role,
        b"P" | b"H1" | b"H2" | b"H3" | b"H4" | b"H5" | b"H6" | b"LI" | b"Link"
    )
}

pub fn is_objr_type(obj: &Object, doc: &Document) -> bool {
    match obj {
        Object::Reference(id) => {
            doc.get_object(*id).ok()
                .and_then(|o| o.as_dict().ok())
                .map(|d| d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"OBJR"))
                .unwrap_or(false)
        }
        Object::Dictionary(d) => {
            d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"OBJR")
        }
        _ => false
    }
}
