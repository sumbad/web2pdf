use std::collections::HashSet;

use lopdf::{Dictionary, Document, Object, ObjectId};

pub fn node_is_nonstruct(doc: &Document, id: ObjectId) -> bool {
    doc.get_object(id)
        .and_then(|o| o.as_dict())
        .map(is_nonstruct)
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

    if let Ok(dict) = doc.get_object(id).and_then(|o| o.as_dict())
        && let Ok(kids) = dict.get(b"K")
    {
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
