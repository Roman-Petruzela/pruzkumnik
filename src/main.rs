mod app;
mod fs_operations;
mod help;
mod types;

fn main() -> std::io::Result<()> {
    app::run()
}
