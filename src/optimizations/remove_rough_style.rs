use super::common::iter::EasyIter;
use crate::node::{Node, RegularNodeType};
use lazy_regex::regex;

/// Parse command into letter and coordinate values
fn parse_coords(cmd: &str) -> (char, Vec<f64>) {
    let letter = cmd.chars().next().unwrap_or('M');
    let re = regex!(r"-?\d+\.?\d*");
    let coords: Vec<f64> = re.find_iter(cmd)
        .filter_map(|m| m.as_str().parse().ok())
        .collect();
    (letter, coords)
}

/// Check if two commands draw roughly the same shape
/// Returns true if they have the same command type and same endpoints
fn commands_are_similar(cmd1: &str, cmd2: &str, tolerance: f64) -> bool {
    let (letter1, coords1) = parse_coords(cmd1);
    let (letter2, coords2) = parse_coords(cmd2);
    
    // Must be same command type
    if letter1 != letter2 {
        return false;
    }
    
    // Must have same number of coordinates
    if coords1.len() != coords2.len() {
        return false;
    }
    
    // For most commands, check if they have the same endpoint
    if coords1.len() >= 2 && coords2.len() >= 2 {
        let len = coords1.len();
        let x1 = coords1[len - 2];
        let y1 = coords1[len - 1];
        let x2 = coords2[len - 2];
        let y2 = coords2[len - 1];
        
        let dx = (x1 - x2).abs();
        let dy = (y1 - y2).abs();
        
        // If endpoints match, these commands draw to the same point
        // (rough style varies control points but keeps endpoints)
        return dx < tolerance && dy < tolerance;
    }
    
    true
}

/// Canonicalize a command by converting it to its simplest form
/// For rough-style deduplication, we want consistent output regardless of control point variations
fn canonicalize_command(cmd: &str) -> String {
    let (letter, coords) = parse_coords(cmd);
    
    if coords.is_empty() {
        return letter.to_string();
    }
    
    match letter {
        // For Bezier curves, extract just the endpoint and create a simple line
        'C' if coords.len() >= 6 => {
            // C command has 6 coords: x1 y1, x2 y2, x3 y3 (endpoint is x3, y3)
            let x = coords[coords.len() - 2];
            let y = coords[coords.len() - 1];
            format!("L{} {}", x, y)
        },
        'S' if coords.len() >= 4 => {
            // S command has 4 coords: x2 y2, x3 y3 (endpoint is x3, y3)
            let x = coords[coords.len() - 2];
            let y = coords[coords.len() - 1];
            format!("L{} {}", x, y)
        },
        'Q' if coords.len() >= 4 => {
            // Q command has 4 coords: x1 y1, x2 y2 (endpoint is x2, y2)
            let x = coords[coords.len() - 2];
            let y = coords[coords.len() - 1];
            format!("L{} {}", x, y)
        },
        'T' if coords.len() >= 2 => {
            // T command has 2 coords: x y (endpoint)
            let x = coords[coords.len() - 2];
            let y = coords[coords.len() - 1];
            format!("L{} {}", x, y)
        },
        // For other commands, keep as-is
        _ => cmd.to_string()
    }
}

/// Deduplicate rough-style path commands
/// Removes consecutive commands that draw roughly the same shape (Mermaid.js rough style)
fn deduplicate_path_data(path_data: &str) -> String {
    let cmd_re = regex!(r"[MLHVCSQTAZ][^MLHVCSQTAZ]*");
    
    // Parse all commands
    let commands: Vec<&str> = cmd_re.captures_iter(path_data)
        .map(|caps| caps.get(0).unwrap().as_str())
        .collect();
    
    if commands.is_empty() {
        return String::new();
    }
    
    // Keep only the first command from each group of similar consecutive commands
    // Output canonicalized versions for consistency
    let mut result = Vec::new();
    let mut i = 0;
    
    while i < commands.len() {
        // Keep current command in canonical form
        result.push(canonicalize_command(commands[i]));
        
        // Skip similar consecutive commands
        let current = commands[i];
        let mut j = i + 1;
        while j < commands.len() && commands_are_similar(current, commands[j], 0.1) {
            j += 1;
        }
        
        i = j;
    }
    
    result.join(" ")
}

fn remove_rough_style_from_node(node: Node) -> Option<Node> {
    match node {
        Node::RegularNode {
            node_type,
            namespace,
            mut attributes,
            children,
        } => {
            // If this is a path element, deduplicate its 'd' attribute
            if matches!(node_type, RegularNodeType::Path) {
                for attribute in &mut attributes {
                    if attribute.name.local_name == "d" {
                        attribute.value = deduplicate_path_data(&attribute.value);
                    }
                }
            }
            
            // Recursively process children
            let processed_children = remove_rough_style(children);
            
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

    #[test]
    fn test_mermaid_rough_style_files_produce_identical_output() {
        // Read the two input files
        let patient1 = std::fs::read_to_string("examples/patient.svg")
            .expect("Failed to read patient.svg");
        let patient2 = std::fs::read_to_string("examples/patient-2.svg")
            .expect("Failed to read patient-2.svg");

        // Parse both files
        let mut parser1 = Parser::new(patient1.as_bytes())
            .expect("Failed to create parser for patient.svg");
        let nodes1 = parser1.parse_document().expect("Failed to parse patient.svg");
        
        let mut parser2 = Parser::new(patient2.as_bytes())
            .expect("Failed to create parser for patient-2.svg");
        let nodes2 = parser2.parse_document().expect("Failed to parse patient-2.svg");

        // Apply the optimization
        let optimized1 = remove_rough_style(nodes1);
        let optimized2 = remove_rough_style(nodes2);

        // Write to strings
        let mut output1 = Vec::new();
        let mut writer1 = SVGWriter::new(&mut output1);
        writer1.write(optimized1).expect("Failed to write output1");

        let mut output2 = Vec::new();
        let mut writer2 = SVGWriter::new(&mut output2);
        writer2.write(optimized2).expect("Failed to write output2");

        // Convert to strings for comparison
        let output1_str = String::from_utf8(output1).expect("Invalid UTF-8 in output1");
        let output2_str = String::from_utf8(output2).expect("Invalid UTF-8 in output2");

        // Debug: compare outputs
        if output1_str != output2_str {
            eprintln!("Output lengths: {} vs {}", output1_str.len(), output2_str.len());
            
            // Find first difference
            let bytes1 = output1_str.as_bytes();
            let bytes2 = output2_str.as_bytes();
            let min_len = bytes1.len().min(bytes2.len());
            
            for i in 0..min_len {
                if bytes1[i] != bytes2[i] {
                    let start = i.saturating_sub(100);
                    let end = (i + 100).min(min_len);
                    eprintln!("\nFirst difference at byte {}:", i);
                    eprintln!("File 1 context: {:?}", String::from_utf8_lossy(&bytes1[start..end]));
                    eprintln!("File 2 context: {:?}", String::from_utf8_lossy(&bytes2[start..end]));
                    break;
                }
            }
        }
        
        // Assert outputs are identical
        assert_eq!(
            output1_str, output2_str,
            "Outputs should be identical for both input files.\nOutput1 length: {}\nOutput2 length: {}",
            output1_str.len(),
            output2_str.len()
        );
    }
}
