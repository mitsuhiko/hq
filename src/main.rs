mod exec;
mod pipeline;

fn main() {
    let mut pipeline = pipeline::Pipeline::new();
    pipeline.add_pass(|pass| {
        pass.filter("ul.menu");
    });
    pipeline.add_pass(|pass| {
        pass.on("a[href]", |sel| {
            sel.rewrite_attribute("href", "^http:", "https:");
            sel.set_inner_content("{{ attributes|tojson }}");
        });
    });

    let mut exec = pipeline.build();
    print!("{}", exec.exec(include_bytes!("../menu.html")));
}
