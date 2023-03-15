use indextree::{Arena, NodeId};

use crate::utils;

use super::tree::{Ast, BaseType, Node, NodeKind, Type};

pub struct TreesitterTranslator {
    arena: Arena<Node>,
    source_code: String,
    tree: tree_sitter::Tree,
}

impl TreesitterTranslator {
    fn new(source_code: String, tree: tree_sitter::Tree) -> TreesitterTranslator {
        TreesitterTranslator {
            arena: Arena::new(),
            source_code,
            tree: tree.into(),
        }
    }

    pub fn translate(source_code: String, tree: tree_sitter::Tree) -> Ast {
        let mut translator = TreesitterTranslator::new(source_code, tree);
        let root_id = translator.parse_root();
        Ast {
            arena: translator.arena,
            root_id: Some(root_id),
        }
    }

    fn parse_root(&mut self) -> NodeId {
        let ast_root = self.arena.new_node(Node {
            kind: NodeKind::Root,
            range: utils::ts_range_to_lsp_range(self.tree.root_node().range()),
            content: self.source_code.clone(),
        });

        // TODO: REMOVE CLONE
        let tree = self.tree.clone();
        let mut cursor = tree.walk();
        for child in tree.root_node().children(&mut cursor) {
            let new_child = match child.kind() {
                "constant_declaration" => self.parse_const_dec(&child),
                _ => None,
            };

            if let Some(new_child) = new_child {
                ast_root.append(new_child, &mut self.arena);
            }
        }

        ast_root
    }

    fn parse_const_dec(&mut self, node: &tree_sitter::Node) -> Option<NodeId> {
        let node_id = self.arena.new_node(Node {
            kind: NodeKind::ConstantDec,
            range: utils::ts_range_to_lsp_range(node.range()),
            content: utils::get_node_text(&node, &self.source_code),
        });

        // Add type node
        node_id.append(
            self.parse_type(&node.child_by_field_name("type").unwrap())
                .unwrap(),
            &mut self.arena,
        );

        // Add name node
        node_id.append(
            self.parse_name(&node.child_by_field_name("name").unwrap())
                .unwrap(),
            &mut self.arena,
        );
        // TODO: Add value node

        Some(node_id)
    }

    fn parse_name(&mut self, node: &tree_sitter::Node) -> Option<NodeId> {
        Some(self.arena.new_node(Node {
            kind: NodeKind::Name,
            range: utils::ts_range_to_lsp_range(node.range()),
            content: utils::get_node_text(&node, &self.source_code),
        }))
    }

    fn parse_type(&mut self, node: &tree_sitter::Node) -> Option<NodeId> {
        let child = node.named_child(0).unwrap();
        let type_type: Type = match child.kind() {
            "base_type" => Type::Base(self.parse_base_type(&child).unwrap()),
            "type_name" => {
                todo!()
            }
            "specialized_type" => {
                todo!()
            }
            "header_stack_type" => {
                todo!()
            }
            "tuple_type" => {
                todo!()
            }
            _ => panic!("{}", node.kind()),
        };

        Some(self.arena.new_node(Node {
            kind: NodeKind::Type(type_type),
            range: utils::ts_range_to_lsp_range(node.range()),
            content: utils::get_node_text(&node, &self.source_code),
        }))
    }

    fn parse_base_type(&self, node: &tree_sitter::Node) -> Result<BaseType, &'static str> {
        let node_text = utils::get_node_text(node, &self.source_code);
        let text = node_text.as_str().trim();

        match text {
            "bool" => Ok(BaseType::Bool),
            "int" => Ok(BaseType::Int),
            "bit" => Ok(BaseType::Bit),
            "string" => Ok(BaseType::String),
            "varbit" => Ok(BaseType::Varbit),
            "error" => Ok(BaseType::Error),
            "match_kind" => Ok(BaseType::MatchKind),
            _ => {
                let child = node.named_child(0).unwrap();
                let size = if child.kind() == "integer" {
                    Some(
                        utils::get_node_text(&child, &self.source_code)
                            .parse::<u32>()
                            .unwrap(),
                    )
                } else {
                    None
                };

                if text.starts_with("int") {
                    Ok(BaseType::SizedInt(size))
                } else if text.starts_with("bit") {
                    Ok(BaseType::SizedBit(size))
                } else if text.starts_with("varbit") {
                    Ok(BaseType::SizedVarbit(size))
                } else {
                    Err("Invalid type")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use indextree::Arena;
    use tree_sitter::{Parser, Tree};
    use tree_sitter_p4::language;

    use crate::{
        ast::tree::{BaseType, Node, NodeKind, Type},
        utils,
    };

    use super::TreesitterTranslator;

    fn get_syntax_tree(source_code: &str) -> Tree {
        let mut parser = Parser::new();
        parser.set_language(language()).unwrap();
        parser.parse(source_code, None).unwrap()
    }

    #[test]
    fn test_const_declaration() {
        let source_code = r#"
            const bit<16> TYPE_IPV4 = 10;
        "#;
        let syntax_tree = get_syntax_tree(source_code);
        let translated_ast =
            TreesitterTranslator::translate(source_code.to_string(), syntax_tree.clone());

        let mut arena: Arena<Node> = Arena::new();
        let mut syntax_node = syntax_tree.root_node();
        let root = arena.new_node(Node {
            kind: NodeKind::Root,
            range: utils::ts_range_to_lsp_range(syntax_node.range()),
            content: utils::get_node_text(&syntax_node, source_code),
        });

        syntax_node = syntax_node.named_child(0).unwrap();
        let constant_syntax_node = syntax_node;
        let constant_dec = arena.new_node(Node {
            kind: NodeKind::ConstantDec,
            range: utils::ts_range_to_lsp_range(syntax_node.range()),
            content: utils::get_node_text(&syntax_node, source_code),
        });
        root.append(constant_dec, &mut arena);

        syntax_node = constant_syntax_node.child_by_field_name("type").unwrap();
        let type_dec = arena.new_node(Node {
            kind: NodeKind::Type(Type::Base(BaseType::SizedBit(Some(16)))),
            range: utils::ts_range_to_lsp_range(syntax_node.range()),
            content: utils::get_node_text(&syntax_node, source_code),
        });
        constant_dec.append(type_dec, &mut arena);

        syntax_node = constant_syntax_node.child_by_field_name("name").unwrap();
        let type_dec = arena.new_node(Node {
            kind: NodeKind::Name,
            range: utils::ts_range_to_lsp_range(syntax_node.range()),
            content: utils::get_node_text(&syntax_node, source_code),
        });
        constant_dec.append(type_dec, &mut arena);

        assert!(translated_ast.arena.eq(&arena))
    }
}
