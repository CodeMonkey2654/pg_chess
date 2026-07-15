use pgrx::prelude::*;

::pgrx::pg_module_magic!(name, version);

mod analysis;
mod api;

#[pg_extern]
fn pg_chess_version() -> &'static str {
    "pg_chess 0.1.0"
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn test_version() {
        assert_eq!("pg_chess 0.1.0", crate::pg_chess_version());
    }
}

#[cfg(any(test, feature = "pg_test"))]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {}
    pub fn postgresql_conf_options() -> Vec<&'static str> {
        vec![]
    }
}
