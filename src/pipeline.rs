use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use lol_html::html_content::{ContentType, Element};
use lol_html::{element, HtmlRewriter, OutputSink, Settings};
use minijinja::context;
use regex::Regex;

use crate::exec::{Exec, ExecState};

pub struct PipelineSink<'h> {
    pass_state: Rc<RefCell<PassState>>,
    forward_to_rewriter: Option<Box<HtmlRewriter<'h, PipelineSink<'h>>>>,
}

impl<'h> OutputSink for PipelineSink<'h> {
    fn handle_chunk(&mut self, chunk: &[u8]) {
        if self
            .pass_state
            .borrow()
            .output_enabled
            .last()
            .copied()
            .unwrap_or(true)
        {
            if let Some(ref mut forward_to) = self.forward_to_rewriter {
                forward_to.write(chunk).unwrap();
            } else {
                ExecState::with(|state| state.write(chunk))
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Pipeline {
    passes: Vec<Pass>,
}

#[derive(Debug)]
pub struct Pass {
    idx: usize,
    selectors: Vec<Rc<Selector>>,
}

#[derive(Debug)]
pub struct Selector {
    selector: String,
    actions: Vec<Action>,
}

#[derive(Debug)]
pub enum Action {
    Filter,
    RewriteAttribute {
        attr: String,
        regex: Regex,
        replacement: String,
    },
    SetInnerContent {
        template: String,
    },
}

impl Pipeline {
    pub fn new() -> Pipeline {
        Pipeline::default()
    }

    pub fn add_pass<F: FnOnce(&mut Pass)>(&mut self, f: F) {
        let mut pass = Pass {
            idx: self.passes.len(),
            selectors: Vec::new(),
        };
        f(&mut pass);
        self.passes.push(pass);
    }

    pub fn build(&self) -> Exec<'_> {
        let mut rewriter = None;
        let mut root_state = None;
        for pass in &self.passes {
            let (prev, state) = pass.build(rewriter);
            rewriter = Some(prev);
            if root_state.is_none() {
                root_state = Some(state);
            }
        }
        Exec {
            rewriter: rewriter.unwrap(),
            root_state: root_state.unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct PassState {
    output_enabled: Vec<bool>,
}

impl Pass {
    pub fn on<F: FnOnce(&mut Selector)>(&mut self, selector: &str, f: F) {
        let mut sel = Selector {
            selector: selector.into(),
            actions: Vec::new(),
        };
        f(&mut sel);
        self.selectors.push(Rc::new(sel));
    }

    pub fn filter(&mut self, selector: &str) {
        self.on(selector, |pass| pass.filter());
    }

    pub fn default_output(&self) -> bool {
        for selector in &self.selectors {
            for action in &selector.actions {
                if let Action::Filter = action {
                    return false;
                }
            }
        }
        true
    }

    pub fn build<'h>(
        &'h self,
        next: Option<HtmlRewriter<'h, PipelineSink<'h>>>,
    ) -> (HtmlRewriter<'_, PipelineSink<'_>>, Rc<RefCell<PassState>>) {
        let mut settings = Settings::default();
        let state = Rc::new(RefCell::new(PassState {
            output_enabled: vec![self.default_output()],
        }));
        for selector in &self.selectors {
            let state = state.clone();
            settings
                .element_content_handlers
                .push(element!(selector.selector, move |el| {
                    let state = state.clone();
                    let selector = selector.clone();
                    for action in &selector.actions {
                        action.enter(el, &mut state.borrow_mut());
                    }
                    el.on_after_end_tag(move |tag| {
                        for action in &selector.actions {
                            action.leave(&tag.name(), &mut state.borrow_mut());
                        }
                        Ok(())
                    })?;
                    Ok(())
                }));
        }
        (
            HtmlRewriter::new(
                settings,
                PipelineSink {
                    pass_state: state.clone(),
                    forward_to_rewriter: next.map(Box::new),
                },
            ),
            state,
        )
    }
}

impl Action {
    pub fn enter(&self, el: &mut Element, state: &mut PassState) {
        match self {
            Action::Filter => {
                state.output_enabled.push(true);
            }
            Action::RewriteAttribute {
                attr,
                regex,
                replacement,
            } => {
                let val = el.get_attribute(attr).unwrap_or_default();
                let rv = regex.replace_all(&val, replacement.as_str());
                el.set_attribute(attr, &rv).unwrap();
            }
            Action::SetInnerContent { template } => el.set_inner_content(
                &ExecState::with(|state| {
                    let attributes = el
                        .attributes()
                        .iter()
                        .map(|x| (x.name(), x.value()))
                        .collect::<BTreeMap<_, _>>();
                    state.render_template(
                        &template,
                        context! {
                            tag => el.tag_name(),
                            attributes,
                        },
                    )
                }),
                ContentType::Html,
            ),
        }
    }

    pub fn leave(&self, tag: &str, state: &mut PassState) {
        match self {
            Action::Filter => {
                state.output_enabled.pop();
            }
            Action::RewriteAttribute { .. } => {}
            Action::SetInnerContent { .. } => {}
        }
    }
}

impl Selector {
    pub fn rewrite_attribute(&mut self, attr: &str, regex: &str, replacement: &str) {
        self.actions.push(Action::RewriteAttribute {
            attr: attr.into(),
            regex: Regex::new(regex).unwrap(),
            replacement: replacement.into(),
        });
    }

    pub fn set_inner_content(&mut self, template: &str) {
        self.actions.push(Action::SetInnerContent {
            template: template.into(),
        });
    }

    pub fn filter(&mut self) {
        Box::new(self.actions.push(Action::Filter));
    }
}
