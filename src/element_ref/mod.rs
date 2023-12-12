//! Element references.

use std::iter::FusedIterator;
use std::ops::Deref;

use ego_tree::iter::{Edge, Traverse};
use ego_tree::NodeRef;
use html5ever::serialize::{serialize, SerializeOpts, TraversalScope};

use crate::node::Element;
use crate::r#trait::TryNextError;
use crate::{Node, Selector};

/// Wrapper around a reference to an element node.
///
/// This wrapper implements the `Element` trait from the `selectors` crate, which allows it to be
/// matched against CSS selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementRef<'a> {
    node: NodeRef<'a, Node>,
}

impl<'a> ElementRef<'a> {
    fn new(node: NodeRef<'a, Node>) -> Self {
        ElementRef { node }
    }

    /// Wraps a `NodeRef` only if it references a `Node::Element`.
    pub fn wrap(node: NodeRef<'a, Node>) -> Option<Self> {
        if node.value().is_element() {
            Some(ElementRef::new(node))
        } else {
            None
        }
    }

    /// Returns the `Element` referenced by `self`.
    pub fn value(&self) -> &'a Element {
        self.node.value().as_element().unwrap()
    }

    /// Returns an iterator over descendent elements matching a selector.
    pub fn select<'b>(&self, selector: &'b Selector) -> Select<'a, 'b> {
        let mut inner = self.traverse();
        inner.next(); // Skip Edge::Open(self).

        Select {
            scope: *self,
            inner,
            selector,
            index: 0,
        }
    }

    fn serialize(&self, traversal_scope: TraversalScope) -> String {
        let opts = SerializeOpts {
            scripting_enabled: false, // It's not clear what this does.
            traversal_scope,
            create_missing_parent: false,
        };
        let mut buf = Vec::new();
        serialize(&mut buf, self, opts).unwrap();
        String::from_utf8(buf).unwrap()
    }

    /// Returns the HTML of this element.
    pub fn html(&self) -> String {
        self.serialize(TraversalScope::IncludeNode)
    }

    /// Returns the inner HTML of this element.
    pub fn inner_html(&self) -> String {
        self.serialize(TraversalScope::ChildrenOnly(None))
    }

    /// Returns the value of an attribute.
    pub fn attr(&self, attr: &str) -> Option<&'a str> {
        self.value().attr(attr)
    }

    /// Returns the value of an attribute of an error if the attribute was not found.
    pub fn try_attr<'b, 'c>(&'c self, attr: &'b str) -> Result<&'a str, AttrNotFoundError<'a, 'b>> {
        self.attr(attr).ok_or_else(|| AttrNotFoundError {
            element: *self,
            attr,
        })
    }

    /// Returns an iterator over descendent text nodes.
    pub fn text(&self) -> Text<'a> {
        Text {
            inner: self.traverse(),
            index: 0,
        }
    }

    /// Iterate over all child nodes which are elements
    ///
    /// # Example
    ///
    /// ```
    /// # use scraper::Html;
    /// let fragment = Html::parse_fragment("foo<span>bar</span><a>baz</a>qux");
    ///
    /// let children = fragment.root_element().child_elements().map(|element| element.value().name()).collect::<Vec<_>>();
    /// assert_eq!(children, ["span", "a"]);
    /// ```
    pub fn child_elements(&self) -> impl Iterator<Item = ElementRef<'a>> {
        self.children().filter_map(ElementRef::wrap)
    }

    /// Iterate over all descendent nodes which are elements
    ///
    /// # Example
    ///
    /// ```
    /// # use scraper::Html;
    /// let fragment = Html::parse_fragment("foo<span><b>bar</b></span><a><i>baz</i></a>qux");
    ///
    /// let descendants = fragment.root_element().descendent_elements().map(|element| element.value().name()).collect::<Vec<_>>();
    /// assert_eq!(descendants, ["html", "span", "b", "a", "i"]);
    /// ```
    pub fn descendent_elements(&self) -> impl Iterator<Item = ElementRef<'a>> {
        self.descendants().filter_map(ElementRef::wrap)
    }
}

impl<'a> Deref for ElementRef<'a> {
    type Target = NodeRef<'a, Node>;
    fn deref(&self) -> &NodeRef<'a, Node> {
        &self.node
    }
}

/// Error to be returned when an attribute is not found.
#[derive(Debug)]
pub struct AttrNotFoundError<'a, 'b> {
    /// Element for which attribute was not found
    pub element: ElementRef<'a>,
    /// Missing attribute
    pub attr: &'b str,
}

/// Iterator over descendent elements matching a selector.
#[derive(Debug, Clone)]
pub struct Select<'a, 'b> {
    scope: ElementRef<'a>,
    inner: Traverse<'a, Node>,
    selector: &'b Selector,
    index: usize,
}

impl<'a, 'b> Iterator for Select<'a, 'b> {
    type Item = ElementRef<'a>;

    fn next(&mut self) -> Option<ElementRef<'a>> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Some(element) = ElementRef::wrap(node) {
                    if self.selector.matches_with_scope(&element, Some(self.scope)) {
                        self.index += 1;
                        return Some(element);
                    }
                }
            }
        }
        None
    }
}

impl FusedIterator for Select<'_, '_> {}

impl<'a, 'b> TryNextError for Select<'a, 'b> {
    type Error = ElementNotFoundError<'a, 'b>;

    fn try_next_err(&mut self) -> Self::Error {
        let Self {
            scope,
            selector,
            index,
            ..
        } = self;
        Self::Error {
            scope: *scope,
            selector,
            index: *index,
        }
    }
}

/// Error to be returned when iterator over descendent elements matching a selector does not find an element.
#[derive(Debug)]
pub struct ElementNotFoundError<'a, 'b> {
    /// Scope where selector was applied
    pub scope: ElementRef<'a>,
    /// Selector that was applied
    pub selector: &'b Selector,
    /// Index where no element was found
    pub index: usize,
}

/// Iterator over descendent text nodes.
#[derive(Debug, Clone)]
pub struct Text<'a> {
    inner: Traverse<'a, Node>,
    index: usize,
}

impl<'a> Iterator for Text<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        for edge in &mut self.inner {
            if let Edge::Open(node) = edge {
                if let Node::Text(ref text) = node.value() {
                    self.index += 1;
                    return Some(&**text);
                }
            }
        }
        None
    }
}

impl<'a> TryNextError for Text<'a> {
    type Error = TextNotFoundError<'a>;

    fn try_next_err(&mut self) -> Self::Error {
        Self::Error {
            root: self.inner.root(),
            index: self.index,
        }
    }
}

/// Error to be returned when iterator over descendent text nodes does not find a node.
#[derive(Debug)]
pub struct TextNotFoundError<'a> {
    /// Root node
    pub root: NodeRef<'a, Node>,
    /// Index where no text node was found
    pub index: usize,
}

mod element;
mod serializable;

#[cfg(test)]
mod tests {
    use crate::html::Html;
    use crate::selector::Selector;

    #[test]
    fn test_scope() {
        let html = r"
            <div>
                <b>1</b>
                <span>
                    <span><b>2</b></span>
                    <b>3</b>
                </span>
            </div>
        ";
        let fragment = Html::parse_fragment(html);
        let sel1 = Selector::parse("div > span").unwrap();
        let sel2 = Selector::parse(":scope > b").unwrap();

        let element1 = fragment.select(&sel1).next().unwrap();
        let element2 = element1.select(&sel2).next().unwrap();
        assert_eq!(element2.inner_html(), "3");
    }
}
