mod actions;
mod app;
mod explorer_fs;
mod help;

fn main() -> std::io::Result<()> {
    app::run()
}
