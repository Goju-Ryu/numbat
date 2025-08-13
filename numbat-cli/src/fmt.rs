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

