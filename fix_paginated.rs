use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PaginatedData<T> {
    total: i64,
    items: Vec<T>,
}
