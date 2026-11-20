//! The paging arguments every list tool takes, and the paging fields it returns.
use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::args::{Args, Range, arg, optional_int};
use crate::config::config::{DEFAULT_PER_PAGE, MAX_PER_PAGE};
use crate::utils::errors::Refused;

/// The page a list call asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageQuery {
    pub page: i64,
    pub per_page: i64,
}

/// Read `page` and `per_page` from a tool's arguments.
pub fn page_query(args: &Args) -> Result<PageQuery, Refused> {
    Ok(PageQuery {
        page: optional_int(arg(args, "page"), "page", Range::min(1))?.unwrap_or(1),
        per_page: optional_int(
            arg(args, "per_page"),
            "per_page",
            Range::between(1, MAX_PER_PAGE),
        )?
        .unwrap_or(DEFAULT_PER_PAGE),
    })
}

/// The paging fields of a list result: the page asked for, and whether GitHub has another.
pub fn paging(query: PageQuery, headers: &BTreeMap<String, String>) -> Map<String, Value> {
    let has_more = headers
        .get("link")
        .map(|link| link.contains("rel=\"next\""))
        .unwrap_or(false);
    let mut map = Map::new();
    map.insert("page".into(), query.page.into());
    map.insert("per_page".into(), query.per_page.into());
    map.insert("has_more".into(), has_more.into());
    map
}
