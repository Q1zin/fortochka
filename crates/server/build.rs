// sqlx::migrate! вшивает migrations/ при компиляции, но сам за папкой
// не следит — просим cargo пересобирать крейт, когда в ней что-то меняется.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
