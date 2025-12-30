use tree_sitter::{Node, Parser};

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

fn print_tree(node: Node, source: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let text = &source[node.start_byte()..node.end_byte()];
    let short_text = if text.len() > 20 {
        format!("{}...", &text[..20].replace('\n', "\\n"))
    } else {
        text.replace('\n', "\\n")
    };

    let eval_marker = if is_evaluatable(node.kind()) {
        " ✓"
    } else {
        ""
    };

    println!(
        "{}[{}]{} '{}'",
        indent,
        node.kind(),
        eval_marker,
        short_text
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(child, source, depth + 1);
    }
}

fn main() {
    let source = "x = 42";

    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("Error loading Python grammar");

    let tree = parser.parse(source, None).unwrap();
    let root = tree.root_node();

    println!("Code: '{}'", source);
    println!("\nAST (✓ = evaluatable):");
    print_tree(root, source, 0);

    println!("\n\nAnalysis:");
    println!("- expression_statement and assignment have same byte range");
    println!("- Both are evaluatable but have different kinds");
    println!("- When selecting 'assignment', parent should be 'expression_statement'");
    println!("- When selecting 'assignment', child should be 'identifier' (x)");
}
