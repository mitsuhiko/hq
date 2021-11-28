use lol_html::HtmlRewriter;
use minijinja::Environment;
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;

use crate::pipeline::{PassState, PipelineSink};

thread_local! {
    static STATE: RefCell<Option<ExecState>> = RefCell::new(None);
}

pub struct ExecState {
    output: Vec<u8>,
}

impl ExecState {
    pub fn with<F: FnOnce(&mut ExecState) -> R, R>(f: F) -> R {
        STATE.with(|state| f(state.borrow_mut().as_mut().unwrap()))
    }

    pub fn write(&mut self, chunk: &[u8]) {
        self.output.extend_from_slice(chunk);
    }

    pub fn render_template<S: Serialize>(&mut self, source: &str, ctx: S) -> String {
        // TODO: error handling and caching
        let mut env = Environment::new();
        env.add_template("tmpl.html", source).unwrap();
        let tmpl = env.get_template("tmpl.html").unwrap();
        tmpl.render(ctx).unwrap()
    }
}

#[derive(Debug)]
pub struct Exec<'a> {
    pub(crate) rewriter: HtmlRewriter<'a, PipelineSink<'a>>,
    pub(crate) root_state: Rc<RefCell<PassState>>,
}

impl<'a> Exec<'a> {
    pub fn exec(&mut self, input: &[u8]) -> String {
        STATE.with(|state| {
            *state.borrow_mut() = Some(ExecState { output: Vec::new() });
        });
        self.rewriter.write(input).unwrap();
        let state = STATE.with(|state| state.borrow_mut().take()).unwrap();
        String::from_utf8(state.output).unwrap()
    }
}
