use itertools::Itertools;
use num_traits::ToPrimitive;

use crate::{
    ast::{BinaryOperator, Expression, Statement, UnaryOperator},
    span::{ByteIndex, Span},
};
use std::collections::HashSet;

// Implementation is shamelessly stolen from Yorick Peterse's article "How to write a code formatter"
// Find it at https://yorickpeterse.com/articles/how-to-write-a-code-formatter/#grouping-nodes

#[derive(Clone)]
enum FormatNode {
    Group(usize, Vec<FormatNode>),
    Nodes(Vec<FormatNode>),
    IfWrap(usize, Box<FormatNode>, Box<FormatNode>),
    Text(String),
    Unicode(String, usize),
    SpaceOrLine,
    Line,
    Indent(Vec<FormatNode>),
    RequiredLine,
}

impl FormatNode {
    fn from_unicode(value: &str) -> FormatNode {
        let len = value.chars().count();
        FormatNode::Unicode(value.to_string(), len)
    }

    fn from_ascii(value: &str) -> FormatNode {
        FormatNode::Text(value.to_string())
    }

    fn width(&self, wrapped: &HashSet<usize>) -> usize {
        match self {
            FormatNode::Group(_, nodes) | FormatNode::Nodes(nodes) | FormatNode::Indent(nodes) => {
                nodes.iter().map(|n| n.width(wrapped)).sum()
            }
            FormatNode::IfWrap(wrap_id, wrap_case, _) if wrapped.contains(wrap_id) => {
                wrap_case.width(wrapped)
            }
            FormatNode::IfWrap(_, _, no_wrap_case) => no_wrap_case.width(wrapped),
            FormatNode::Text(str) => str.len(),
            FormatNode::Unicode(_, len) => *len,
            FormatNode::SpaceOrLine => 1,
            FormatNode::Line => 0,
            FormatNode::RequiredLine => 0,
        }
    }
}

enum Wrap {
    Enable,
    Detect,
}

impl Wrap {
    fn enable(&self) -> bool {
        match self {
            Wrap::Enable => true,
            Wrap::Detect => false,
        }
    }
}

const INDENT: &str = "    ";

struct Generator {
    buffer: String,
    indent: usize,
    size: usize,
    max: usize,
    wrapped: HashSet<usize>,
}

impl Generator {
    pub fn new(max: usize) -> Generator {
        Generator {
            buffer: String::new(),
            indent: 0,
            size: 0,
            max: max,
            wrapped: HashSet::new(),
        }
    }

    fn generate(&mut self, node: FormatNode) -> String {
        self.node(&node, &Wrap::Detect);
        let result = self.buffer.clone();
        self.buffer.clear();
        self.indent = 0;
        self.size = 0;
        self.wrapped.clear();
        result
    }

    fn node(&mut self, node: &FormatNode, wrap: &Wrap) {
        match node {
            FormatNode::Group(id, nodes) => {
                let width = node.width(&self.wrapped);
                let wrap = if self.size + width > self.max {
                    self.wrapped.insert(*id);
                    Wrap::Enable
                } else {
                    Wrap::Detect
                };

                self.nodes(nodes, &wrap);
            }
            FormatNode::Nodes(nodes) => self.nodes(nodes, wrap),
            FormatNode::IfWrap(id, wrap_node, _) if self.wrapped.contains(&id) => {
                self.node(&wrap_node, wrap)
            }
            FormatNode::IfWrap(_, _, no_wrap_node) => self.node(&no_wrap_node, wrap),
            FormatNode::Text(str) => self.text(str, str.len()),
            FormatNode::Unicode(str, width) => self.text(str, *width),
            FormatNode::SpaceOrLine | FormatNode::Line if wrap.enable() => self.new_line(),
            FormatNode::SpaceOrLine => self.text(" ", 1),
            FormatNode::Line => {} // No newline needed when wrap isn't enabled
            FormatNode::Indent(nodes) if wrap.enable() => {
                self.size += INDENT.len();
                self.indent += 1;
                self.buffer.push_str(INDENT);
                self.nodes(nodes, wrap);
                self.indent -= 1;
            }
            FormatNode::Indent(nodes) => self.nodes(nodes, wrap),
            FormatNode::RequiredLine => self.new_line(),
        }
    }

    fn nodes(&mut self, nodes: &Vec<FormatNode>, wrap: &Wrap) {
        nodes.iter().for_each(|n| self.node(n, wrap))
    }

    fn text(&mut self, value: &str, chars: usize) {
        self.size += chars;
        self.buffer.push_str(value);
    }

    fn new_line(&mut self) {
        self.size = INDENT.len() * self.indent;
        self.buffer.push('\n');
        self.buffer.push_str(&INDENT.repeat(self.indent));
    }
}

struct Builder<'a> {
    id: usize,
    source: &'a str,
    prev_span: Span,
}

impl<'a> Builder<'a> {
    fn new(source: &'a str) -> Builder<'a> {
        Builder {
            id: 0,
            source,
            prev_span: Span {
                start: ByteIndex(0),
                end: ByteIndex(0),
                code_source_id: 0,
            },
        }
    }

    fn build(&'a mut self, ast: Vec<Statement<'a>>) -> FormatNode {
        let format_nodes = Itertools::intersperse(
            ast.iter().map(|n| self.build_statement(n)),
            FormatNode::Nodes(vec![FormatNode::RequiredLine, FormatNode::RequiredLine]),
        )
        .collect();

        let remaining_text = &self.source[self.prev_span.end.as_usize()..].trim();

        FormatNode::Nodes(vec![
            FormatNode::Nodes(format_nodes),
            FormatNode::RequiredLine,
            FormatNode::from_unicode(remaining_text),
            FormatNode::RequiredLine,
        ])
    }

    fn new_id(&mut self) -> usize {
        self.id += 1;
        self.id
    }

    fn with_comments(&mut self, current_span: Span, node: FormatNode) -> FormatNode {
        if self.prev_span.end.as_usize() <= current_span.start.as_usize() {
            let text =
                self.source[self.prev_span.end.as_usize()..current_span.start.as_usize()].trim();
            self.prev_span = current_span;
            if text.len() > 0 {
                let nodes = Itertools::intersperse(
                    text.lines().map(|line| FormatNode::from_unicode(line)),
                    FormatNode::SpaceOrLine,
                )
                .collect();

                FormatNode::Nodes(vec![
                    FormatNode::Line,
                    FormatNode::Nodes(nodes),
                    FormatNode::RequiredLine,
                    node,
                ])
            } else {
                node
            }
        } else {
            node
        }
    }

    fn build_text_from_span(&mut self, span: &Span) -> FormatNode {
        let text = self.source[span.end.as_usize()..span.start.as_usize()].trim();
        self.with_comments(*span, FormatNode::from_unicode(text))
    }

    fn skip(&mut self, token: &str) {
        let mut rest = &self.source[self.prev_span.end.as_usize()..];
        let mut end = self.prev_span.end;

        while !rest.starts_with(token) {
            rest = &rest[1..];
            end += 1;
        }

        self.prev_span = Span {
            start: self.prev_span.end,
            end: end + token.len().to_u32().unwrap(),
            code_source_id: self.prev_span.code_source_id,
        }
    }

    fn build_statement(&mut self, node: &Statement<'a>) -> FormatNode {
        match node {
            Statement::Expression(expr) => self.build_expression(expr),
            Statement::DefineVariable(define_variable) => todo!(),
            Statement::DefineFunction {
                function_name_span,
                function_name,
                type_parameters,
                parameters,
                body,
                local_variables,
                return_type_annotation,
                decorators,
            } => todo!(),
            Statement::DefineDimension(span, _, type_expressions) => todo!(),
            Statement::DefineBaseUnit(span, _, type_expression, decorators) => todo!(),
            Statement::DefineDerivedUnit {
                identifier_span,
                identifier,
                expr,
                type_annotation_span,
                type_annotation,
                decorators,
            } => todo!(),
            Statement::ProcedureCall(span, procedure_kind, expressions) => todo!(),
            Statement::ModuleImport(span, module_path_borrowed) => todo!(),
            Statement::DefineStruct {
                struct_name_span,
                struct_name,
                fields,
            } => todo!(),
        }
    }

    fn build_expression(&mut self, expr: &Expression<'a>) -> FormatNode {
        match expr {
            Expression::Scalar(span, number) => {
                let num = self.source[span.start.as_usize()..span.end.as_usize()].to_string();

                let scalar_node = if num[1..].starts_with(['x', 'o', 'b']) {
                    FormatNode::Text(num)
                } else {
                    FormatNode::from_ascii(&number.pretty_print())
                };

                self.with_comments(*span, scalar_node)
            }
            Expression::Identifier(span, name) => {
                self.with_comments(*span, FormatNode::from_unicode(name))
            }
            Expression::UnitIdentifier(span, prefix, name, _) => self.with_comments(
                *span,
                FormatNode::from_unicode(&(prefix.as_string_short() + name)),
            ),
            Expression::TypedHole(span) => self.build_text_from_span(span),
            Expression::UnaryOperator { op, expr, span_op } => match op {
                UnaryOperator::Factorial(count) => {
                    let expr_node = self.build_expression(expr);
                    self.with_comments(
                        *span_op,
                        FormatNode::Nodes(vec![
                            expr_node,
                            FormatNode::from_ascii(&"!".repeat(count.get())),
                        ]),
                    )
                }
                UnaryOperator::Negate => FormatNode::Nodes(vec![
                    self.with_comments(*span_op, FormatNode::Nodes(vec![])),
                    FormatNode::from_ascii("-"),
                    self.build_expression(expr),
                ]),
                UnaryOperator::LogicalNeg => FormatNode::Nodes(vec![
                    self.with_comments(*span_op, FormatNode::Nodes(vec![])),
                    FormatNode::from_ascii("!"),
                    self.build_expression(expr),
                ]),
            },
            Expression::BinaryOperator {
                op,
                lhs,
                rhs,
                span_op,
            } => {
                let lhs_node = self.build_expression(lhs);
                let op_node: FormatNode = Builder::format_binary_operator(op);

                if let Some(span) = span_op {
                    self.prev_span = *span;
                };

                let rhs_node = self.build_expression(rhs);

                self.with_comments(
                    lhs.full_span().extend(&rhs.full_span()),
                    FormatNode::Nodes(vec![lhs_node, op_node, rhs_node]),
                )
            }
            Expression::FunctionCall(span, span1, expression, expressions) => todo!(),
            Expression::Boolean(span, bool) => self.with_comments(
                *span,
                if *bool {
                    FormatNode::from_ascii("true")
                } else {
                    FormatNode::from_ascii("false")
                },
            ),
            Expression::String(span, string_parts) => todo!(), //TODO Make helper function to handle string parts
            Expression::Condition(span, condition, then_expr, else_expr) => {
                let comment_node = self.with_comments(
                    Span {
                        start: span.start,
                        end: span.start,
                        code_source_id: span.code_source_id,
                    },
                    FormatNode::Nodes(vec![]),
                );

                self.skip("if");
                let condition_node = self.build_expression(condition);
                self.skip("then");
                let then_expr_node = self.build_expression(then_expr);
                self.skip("else");
                let else_expr_node = self.build_expression(else_expr);

                FormatNode::Nodes(vec![
                    comment_node,
                    FormatNode::Group(
                        self.new_id(),
                        vec![
                            FormatNode::from_ascii("if "),
                            condition_node,
                            FormatNode::SpaceOrLine,
                            FormatNode::Group(
                                self.new_id(),
                                vec![
                                    FormatNode::from_ascii("then"),
                                    FormatNode::SpaceOrLine,
                                    FormatNode::Indent(vec![then_expr_node]),
                                ],
                            ),
                            FormatNode::SpaceOrLine,
                            FormatNode::Group(
                                self.new_id(),
                                vec![
                                    FormatNode::from_ascii("else"),
                                    FormatNode::SpaceOrLine,
                                    FormatNode::Indent(vec![else_expr_node]),
                                ],
                            ),
                        ],
                    ),
                ])
            }
            Expression::InstantiateStruct {
                full_span,
                ident_span,
                name,
                fields,
            } => todo!(),
            Expression::AccessField(span, span1, expression, _) => todo!(),
            Expression::List(span, expressions) => todo!(),
        }
    }

    fn format_binary_operator(op: &BinaryOperator) -> FormatNode {
        match op {
            BinaryOperator::Add => FormatNode::from_ascii(" + "),
            BinaryOperator::Sub => FormatNode::from_ascii(" - "),
            BinaryOperator::Mul => FormatNode::from_ascii(" * "),
            BinaryOperator::Div => FormatNode::from_ascii(" / "),
            BinaryOperator::Power => FormatNode::from_ascii(" ^ "),
            BinaryOperator::ConvertTo => FormatNode::from_ascii(" -> "),
            BinaryOperator::LessThan => FormatNode::from_ascii(" < "),
            BinaryOperator::GreaterThan => FormatNode::from_ascii(" > "),
            BinaryOperator::LessOrEqual => FormatNode::from_ascii(" <= "),
            BinaryOperator::GreaterOrEqual => FormatNode::from_ascii(" >= "),
            BinaryOperator::Equal => FormatNode::from_ascii(" == "),
            BinaryOperator::NotEqual => FormatNode::from_ascii(" != "),
            BinaryOperator::LogicalAnd => FormatNode::from_ascii(" && "),
            BinaryOperator::LogicalOr => FormatNode::from_ascii(" || "),
        }
    }
}

pub fn fmt(source: &str, ast: Vec<Statement>, max_width: usize) -> String {
    let format_tree = Builder::new(source).build(ast);
    Generator::new(max_width).generate(format_tree)
}
