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
