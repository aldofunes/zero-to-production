use tera::Tera;

pub fn init_tera() -> Tera {
    Tera::new("templates/**/*").expect("Failed to compile html templates")
}
