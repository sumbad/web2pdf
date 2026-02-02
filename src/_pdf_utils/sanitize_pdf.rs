use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::HashSet;

use super::helpers::*;

pub fn sanitize_pdf(doc: &mut Document) -> anyhow::Result<()> {
    let root_id = find_struct_tree_root_id(doc).ok_or_else(|| anyhow::anyhow!("No root"))?;

    let mut all_nodes = Vec::new();
    collect_all_node_ids(doc, root_id, &mut all_nodes, &mut HashSet::new());

    // IMPORTANT: Process in REVERSE order (from leaves to root)
    // This allows collapsing nested NonStruct in a single clean pass
    for node_id in all_nodes.into_iter().rev() {
        let role = get_node_role(doc, node_id).unwrap_or_default();

        dissolve_nonstruct_in_node(doc, node_id)?;

        // Clean links separately (remove OBJR)
        if role == b"Link" {
            remove_objr_from_link(doc, node_id)?;
        }
    }

    Ok(())
}

fn dissolve_nonstruct_in_node(doc: &mut Document, parent_id: ObjectId) -> anyhow::Result<()> {
    // Get the current object. If it's a NonStruct, we ignore it,
    // as we'll process it when we're at its parent level.
    let Ok(obj) = doc.get_object(parent_id) else {
        return Ok(());
    };
    let dict = obj.as_dict()?.clone();

    // if is_nonstruct(&dict) {
    //     return Ok(());
    // }

    let role = get_node_role(doc, parent_id).unwrap_or_else(|| b"Unknown".to_vec());
    let role_str = String::from_utf8_lossy(&role).to_string();

    let kids = match dict.get(b"K") {
        Ok(Object::Array(arr)) => arr.clone(),
        Ok(obj) => vec![obj.clone()],
        _ => return Ok(()),
    };

    let mut new_kids = Vec::new();
    let mut was_changed = false;

    for (i, kid) in kids.iter().enumerate() {
        if let Some(kid_id) = kid.as_reference().ok() {
            if node_is_nonstruct(doc, kid_id) {
                let kid_dict = doc.get_object(kid_id)?.as_dict()?.clone();
                let kid_pg = kid_dict.get(b"Pg").ok().cloned();

                tracing::debug!(
                    "🔍 Dissolving NonStruct {:?} (child #{} of {} {:?})",
                    kid_id,
                    i,
                    role_str,
                    parent_id
                );

                let grandchildren = match kid_dict.get(b"K") {
                    Ok(Object::Array(arr)) => arr.clone(),
                    Ok(obj) => vec![obj.clone()],
                    _ => vec![],
                };

                for gc in grandchildren {
                    match gc {
                        Object::Integer(mcid) => {
                            // If we extract a bare MCID, wrap it in an MCR dictionary,
                            // so we don't lose the page binding (Pg)
                            if let Some(pg) = &kid_pg {
                                let mut mcr = Dictionary::new();
                                mcr.set("Type", Object::Name(b"MCR".to_vec()));
                                mcr.set("Pg", pg.clone());
                                mcr.set("MCID", Object::Integer(mcid));
                                new_kids.push(Object::Dictionary(mcr));
                            } else {
                                new_kids.push(Object::Integer(mcid));
                            }
                        }
                        Object::Reference(gc_id) => {
                            // If we extract a tag (P, Link, etc.), update its parent
                            set_parent_link(doc, gc_id, parent_id);

                            // If the tag doesn't have its own page, but NonStruct had one - pass it to the tag
                            if let Ok(Object::Dictionary(gc_dict)) = doc.get_object_mut(gc_id) {
                                if !gc_dict.has(b"Pg") {
                                    if let Some(pg) = &kid_pg {
                                        gc_dict.set("Pg", pg.clone());
                                    }
                                }
                            }
                            new_kids.push(Object::Reference(gc_id));
                        }
                        _ => new_kids.push(gc),
                    }
                }
                was_changed = true;
                continue;
            }
        }
        new_kids.push(kid.clone());
    }

    if was_changed {
        let mut updated_dict = dict;
        if new_kids.is_empty() {
            updated_dict.remove(b"K");
        } else if new_kids.len() == 1 {
            updated_dict.set("K", new_kids[0].clone());
        } else {
            updated_dict.set("K", Object::Array(new_kids));
        }
        doc.objects
            .insert(parent_id, Object::Dictionary(updated_dict));
        // tracing::info!(
        //     "✨ Finished dissolving kids for {} {:?}",
        //     role_str,
        //     parent_id
        // );
    }

    Ok(())
}

fn remove_objr_from_link(doc: &mut Document, link_id: ObjectId) -> anyhow::Result<()> {
    let mut dict = doc.get_object(link_id)?.as_dict()?.clone();

    let kids = match dict.get(b"K") {
        Ok(Object::Array(arr)) => arr.clone(),
        Ok(obj) => vec![obj.clone()],
        _ => return Ok(()),
    };

    let mut new_kids = Vec::new();
    let mut changed = false;

    for kid in kids {
        let is_objr = match &kid {
            Object::Reference(id) => {
                // Check the object we're referencing
                if let Ok(obj) = doc.get_object(*id) {
                    if let Ok(d) = obj.as_dict() {
                        d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"OBJR")
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Object::Dictionary(d) => d.get(b"Type").and_then(|t| t.as_name()).ok() == Some(b"OBJR"),
            _ => false,
        };

        if is_objr {
            changed = true;
            tracing::debug!("✂️ Removed OBJR child from Link {:?}", link_id);
            continue; // Don't add OBJR to the new children list
        }
        new_kids.push(kid);
    }

    if changed {
        if new_kids.is_empty() {
            dict.remove(b"K");
        } else if new_kids.len() == 1 {
            dict.set("K", new_kids[0].clone());
        } else {
            dict.set("K", Object::Array(new_kids));
        }
        doc.objects.insert(link_id, Object::Dictionary(dict));
    }

    Ok(())
}
