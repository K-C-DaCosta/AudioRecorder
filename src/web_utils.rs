use web_sys::{Element, HtmlCollection};

pub struct ParentIter {
    elem: Element,
}
impl ParentIter {
    pub fn new<T>(e: T) -> Self
    where
        T: AsRef<Element>,
    {
        Self {
            elem: e.as_ref().clone(),
        }
    }
}
impl Iterator for ParentIter {
    type Item = Element;
    fn next(&mut self) -> Option<Self::Item> {
        self.elem.parent_element().map(|e| {
            self.elem = e.clone();
            e
        })
    }
}

pub struct DomIter {
    html: HtmlCollection,
    idx: u32,
}

impl DomIter {
    pub fn new(c: HtmlCollection) -> Self {
        Self { html: c, idx: 0 }
    }
    pub fn by_class_name<T>(element: T, names: &str) -> DomIter
    where
        T: AsRef<Element>,
    {
        DomIter::new(element.as_ref().get_elements_by_class_name(names))
    }

    pub fn by_tag_name<T>(element: T, names: &str) -> DomIter
    where
        T: AsRef<Element>,
    {
        DomIter::new(element.as_ref().get_elements_by_tag_name(names))
    }
}

impl Iterator for DomIter {
    type Item = Element;
    fn next(&mut self) -> Option<Self::Item> {
        let elem = self.html.get_with_index(self.idx);
        self.idx += 1;
        elem
    }
}
