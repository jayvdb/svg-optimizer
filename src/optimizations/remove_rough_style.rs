use super::common::iter::EasyIter;
use crate::node::{Node, RegularNodeType};

fn remove_rough_style_from_node(node: Node) -> Option<Node> {
    match node {
        Node::RegularNode {
            node_type,
            namespace,
            attributes,
            children,
        } => {
            // Check if this is a rough-style outer-path group
            let is_outer_path = attributes.iter().any(|attribute| {
                attribute.name.local_name == "class" && attribute.value.contains("outer-path")
            });
            
            let processed_children = if is_outer_path {
                // This is a rough-style container with duplicate paths
                // Keep only the first path (filled base), skip rough stroke overlays
                let mut kept_children = Vec::new();
                let mut found_first_path = false;
                
                for child in children {
                    match &child {
                        Node::RegularNode { node_type: RegularNodeType::Path, .. } => {
                            if !found_first_path {
                                // Keep the first path (filled base shape)
                                kept_children.push(child);
                                found_first_path = true;
                            }
                            // Skip subsequent paths (rough stroke overlays)
                        }
                        _ => {
                            // Keep all non-path children, recursively process them
                            kept_children.push(child);
                        }
                    }
                }
                
                remove_rough_style(kept_children)
            } else {
                // Not an outer-path group, recursively process children normally
                remove_rough_style(children)
            };
            
            Some(Node::RegularNode {
                node_type,
                namespace,
                attributes,
                children: processed_children,
            })
        }
        childless_node @ Node::ChildlessNode { .. } => Some(childless_node),
    }
}

pub(crate) fn remove_rough_style(nodes: Vec<Node>) -> Vec<Node> {
    nodes.filter_map_to_vec(remove_rough_style_from_node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimizations::common::test::test_optimize;
    use crate::parser::Parser;
    use crate::writer::SVGWriter;

    test_optimize!(
        test_removes_rough_style_duplicate_paths,
        remove_rough_style,
        r#"
        <svg xmlns="http://www.w3.org/2000/svg"><g class="outer-path"><path d="M0"/><path d="M1"/></g></svg>
        "#,
        r#"
        <svg xmlns="http://www.w3.org/2000/svg"><g class="outer-path"><path d="M0"/></g></svg>"#
    );
}
