use std::collections::HashSet;

// Implementation is shamelessly stolen from Yorick Peterse's article "How to write a code formatter" 
// Find it at https://yorickpeterse.com/articles/how-to-write-a-code-formatter/#grouping-nodes

pub enum FormatNode<'a> {
    Group(usize, Vec<FormatNode<'a>>),
    Nodes(Vec<FormatNode<'a>>),
    IfWrap(usize, Box<FormatNode<'a>>, Box<FormatNode<'a>>),
    Text(&'a str),
    Unicode(&'a str, usize),
    SpaceOrLine,
    Line,
    Indent(Vec<FormatNode<'a>>),
}

impl FormatNode<'_> {
    fn from_unicode(value: &str) -> FormatNode<'_> {
        let len = value.chars().count();
        FormatNode::Unicode(value, len)
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

pub struct Generator {
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

    pub fn generate(&mut self, node: FormatNode) -> String {
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
