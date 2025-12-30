use tree_sitter::{Node, Parser, Tree};

/// Initialize the Python parser
pub fn create_parser() -> Parser {
    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("Error loading Python grammar");
    parser
}

/// Represents a selected node in the AST
#[derive(Clone, Debug)]
pub struct SelectedNode {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize, // 0-indexed
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub kind: String,
    pub text: String,
}

impl SelectedNode {
    fn from_node(node: Node, source: &str) -> Self {
        let start = node.start_position();
        let end = node.end_position();
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: start.row,
            end_line: end.row,
            start_col: start.column,
            end_col: end.column,
            kind: node.kind().to_string(),
            text: source[node.start_byte()..node.end_byte()].to_string(),
        }
    }
}

/// Check if a node type is evaluatable (can be meaningfully evaluated by the debugger)
fn is_evaluatable(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "call"
            | "attribute"
            | "subscript"
            | "binary_operator"
            | "unary_operator"
            | "comparison_operator"
            | "boolean_operator"
            | "list"
            | "dictionary"
            | "tuple"
            | "string"
            | "integer"
            | "float"
            | "true"
            | "false"
            | "none"
            | "assignment"
            | "augmented_assignment"
            | "expression_statement"
            | "parenthesized_expression"
            | "list_comprehension"
            | "dictionary_comprehension"
            | "set_comprehension"
            | "generator_expression"
            | "conditional_expression"
            | "lambda"
    )
}

/// Find the first evaluatable node on a given line
pub fn find_first_evaluatable_on_line(
    tree: &Tree,
    source: &str,
    line: usize,
) -> Option<SelectedNode> {
    let root = tree.root_node();
    find_first_evaluatable_on_line_recursive(root, source, line)
}

fn find_first_evaluatable_on_line_recursive(
    node: Node,
    source: &str,
    line: usize,
) -> Option<SelectedNode> {
    // Check if this node is on the target line
    let start_line = node.start_position().row;
    let end_line = node.end_position().row;

    if start_line > line {
        return None; // Past the target line
    }

    if end_line < line {
        return None; // Before the target line
    }

    // This node spans the target line
    // First, try to find evaluatable nodes in children (prefer more specific)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_first_evaluatable_on_line_recursive(child, source, line) {
            return Some(found);
        }
    }

    // If no child is evaluatable on this line, check if this node is evaluatable
    if is_evaluatable(node.kind()) && start_line == line {
        return Some(SelectedNode::from_node(node, source));
    }

    None
}

/// Find node by byte range (used to relocate a node after navigation)
fn find_node_by_range(root: Node, start_byte: usize, end_byte: usize) -> Option<Node> {
    if root.start_byte() == start_byte && root.end_byte() == end_byte {
        return Some(root);
    }

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.start_byte() <= start_byte && child.end_byte() >= end_byte {
            if let Some(found) = find_node_by_range(child, start_byte, end_byte) {
                return Some(found);
            }
        }
    }
    None
}

/// Get the parent evaluatable node
pub fn get_parent_node(tree: &Tree, source: &str, current: &SelectedNode) -> Option<SelectedNode> {
    let root = tree.root_node();
    let node = find_node_by_range(root, current.start_byte, current.end_byte)?;

    let mut parent = node.parent();
    while let Some(p) = parent {
        // Check if this is a different node (not just different start, but different range or kind)
        let is_different = p.start_byte() != current.start_byte
            || p.end_byte() != current.end_byte
            || p.kind() != current.kind;

        if is_evaluatable(p.kind()) && is_different {
            return Some(SelectedNode::from_node(p, source));
        }
        parent = p.parent();
    }
    None
}

/// Get the first evaluatable child node
pub fn get_first_child_node(
    tree: &Tree,
    source: &str,
    current: &SelectedNode,
) -> Option<SelectedNode> {
    let root = tree.root_node();
    let node = find_node_by_range(root, current.start_byte, current.end_byte)?;

    find_first_evaluatable_child(node, source)
}

fn find_first_evaluatable_child(node: Node, source: &str) -> Option<SelectedNode> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_evaluatable(child.kind()) {
            return Some(SelectedNode::from_node(child, source));
        }
        // Recurse into non-evaluatable children to find evaluatable descendants
        if let Some(found) = find_first_evaluatable_child(child, source) {
            return Some(found);
        }
    }
    None
}

/// Get the next sibling evaluatable node
pub fn get_next_sibling(tree: &Tree, source: &str, current: &SelectedNode) -> Option<SelectedNode> {
    let root = tree.root_node();
    let node = find_node_by_range(root, current.start_byte, current.end_byte)?;

    // First try direct siblings
    let mut sibling = node.next_sibling();
    while let Some(s) = sibling {
        if is_evaluatable(s.kind()) {
            return Some(SelectedNode::from_node(s, source));
        }
        // Check if there's an evaluatable child in the sibling
        if let Some(found) = find_first_evaluatable_child(s, source) {
            return Some(found);
        }
        sibling = s.next_sibling();
    }

    // If no next sibling, try parent's next sibling (uncle)
    if let Some(parent) = node.parent() {
        let mut uncle = parent.next_sibling();
        while let Some(u) = uncle {
            if is_evaluatable(u.kind()) {
                return Some(SelectedNode::from_node(u, source));
            }
            if let Some(found) = find_first_evaluatable_child(u, source) {
                return Some(found);
            }
            uncle = u.next_sibling();
        }
    }

    None
}

/// Get the previous sibling evaluatable node
pub fn get_prev_sibling(tree: &Tree, source: &str, current: &SelectedNode) -> Option<SelectedNode> {
    let root = tree.root_node();
    let node = find_node_by_range(root, current.start_byte, current.end_byte)?;

    // First try direct siblings
    let mut sibling = node.prev_sibling();
    while let Some(s) = sibling {
        if is_evaluatable(s.kind()) {
            return Some(SelectedNode::from_node(s, source));
        }
        // Check if there's an evaluatable child in the sibling (prefer last one)
        if let Some(found) = find_last_evaluatable_child(s, source) {
            return Some(found);
        }
        sibling = s.prev_sibling();
    }

    // If no prev sibling, try parent's prev sibling
    if let Some(parent) = node.parent() {
        let mut uncle = parent.prev_sibling();
        while let Some(u) = uncle {
            if is_evaluatable(u.kind()) {
                return Some(SelectedNode::from_node(u, source));
            }
            if let Some(found) = find_last_evaluatable_child(u, source) {
                return Some(found);
            }
            uncle = u.prev_sibling();
        }
    }

    None
}

fn find_last_evaluatable_child(node: Node, source: &str) -> Option<SelectedNode> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    for child in children.into_iter().rev() {
        // Recurse first (prefer deepest last child)
        if let Some(found) = find_last_evaluatable_child(child, source) {
            return Some(found);
        }
        if is_evaluatable(child.kind()) {
            return Some(SelectedNode::from_node(child, source));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = create_parser();
        assert!(parser.language().is_some());
    }

    #[test]
    fn test_find_evaluatable_on_line() {
        let mut parser = create_parser();
        let source = "a = foo()\nb = bar()";
        let tree = parser.parse(source, None).unwrap();

        // Line 0 should find 'a' or 'foo()' or the assignment
        let node = find_first_evaluatable_on_line(&tree, source, 0);
        assert!(node.is_some());
        let n = node.unwrap();
        println!("Found: {} '{}'", n.kind, n.text);
    }

    #[test]
    fn test_navigation() {
        let mut parser = create_parser();
        let source = "a = foo()\nb = bar()";
        let tree = parser.parse(source, None).unwrap();

        let first = find_first_evaluatable_on_line(&tree, source, 0).unwrap();
        println!("First: {} '{}'", first.kind, first.text);

        if let Some(parent) = get_parent_node(&tree, source, &first) {
            println!("Parent: {} '{}'", parent.kind, parent.text);
        }

        if let Some(sibling) = get_next_sibling(&tree, source, &first) {
            println!("Next sibling: {} '{}'", sibling.kind, sibling.text);
        }
    }

    #[test]
    fn test_parent_child_navigation() {
        let mut parser = create_parser();
        let source = "x = 42";
        let tree = parser.parse(source, None).unwrap();

        // Find first evaluatable (should be assignment or identifier)
        let first = find_first_evaluatable_on_line(&tree, source, 0).unwrap();
        println!(
            "\nFirst: {} '{}' ({}:{} to {}:{})",
            first.kind,
            first.text,
            first.start_line,
            first.start_col,
            first.end_line,
            first.end_col
        );

        // Try to find parent
        if let Some(parent) = get_parent_node(&tree, source, &first) {
            println!(
                "Parent: {} '{}' ({}:{} to {}:{})",
                parent.kind,
                parent.text,
                parent.start_line,
                parent.start_col,
                parent.end_line,
                parent.end_col
            );

            // Try to find child of parent (should get back something)
            if let Some(child) = get_first_child_node(&tree, source, &parent) {
                println!("Child of parent: {} '{}'", child.kind, child.text);
                assert!(true, "Found child successfully");
            } else {
                panic!("No child found for parent!");
            }
        } else {
            panic!("No parent found!");
        }

        // Try to find child of first
        if let Some(child) = get_first_child_node(&tree, source, &first) {
            println!("Child of first: {} '{}'", child.kind, child.text);
        } else {
            println!("No child for first node (expected if it's a leaf like identifier)");
        }
    }
}
