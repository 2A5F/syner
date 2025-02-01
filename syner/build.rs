use slint_build::CompilerConfiguration;

fn main() {
    slint_build::compile_with_config(
        "ui/app-window.slint",
        CompilerConfiguration::new().with_style("fluent".into()),
    )
    .expect("Slint build failed");
}
