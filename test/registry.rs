//! Ported from test/registry.test.ts, plus the golden `tools/list` comparison.
use lxagents_github::controllers::{self, build_registry};
use lxagents_github::models::tool::ToolSpec;
use regex::Regex;
use serde_json::Value;

use crate::surface::{NO_REPO_ARGUMENT, kind_hints, kind_of, tool_names};

#[test]
fn the_tools_registered_are_exactly_the_surface() {
    let mut names: Vec<_> = controllers::tools().iter().map(|t| t.name).collect();
    names.sort();
    assert_eq!(names, tool_names());
    assert_eq!(
        controllers::get("repo_create").map(|t| t.name),
        Some("repo_create")
    );
    assert!(controllers::get("nope").is_none());
}

#[test]
fn every_tool_module_is_a_tool_and_every_tool_is_a_module() {
    let mut files: Vec<String> =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src/controllers"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|f| f.ends_with(".rs") && f != "mod.rs")
            .map(|f| f.trim_end_matches(".rs").to_string())
            .collect();
    files.sort();
    let mut names: Vec<String> = controllers::tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    names.sort();
    assert_eq!(names, files);
}

#[test]
fn every_tool_carries_a_description_and_an_output_schema() {
    for tool in controllers::tools() {
        let d = &tool.def;
        assert!(
            d.description.len() > 20,
            "{} has no useful description",
            d.name
        );
        assert!(
            d.description.chars().count() <= 600,
            "{} description is too long",
            d.name
        );
        assert!(
            !d.output_schema["properties"]
                .as_object()
                .unwrap()
                .is_empty(),
            "{} has no output schema",
            d.name
        );
    }
}

#[test]
fn every_tools_annotations_match_its_kind() {
    for tool in controllers::tools() {
        let kind =
            kind_of(tool.name).unwrap_or_else(|| panic!("{} is not in surface.rs", tool.name));
        let h = tool.def.annotations;
        assert_eq!(
            (h.read_only, h.destructive, h.idempotent, h.open_world),
            kind_hints(kind),
            "{}",
            tool.name
        );
    }
}

#[test]
fn every_tool_but_two_takes_a_required_repo() {
    for tool in controllers::tools() {
        let schema = &tool.def.input_schema;
        let has = schema["properties"].get("repo").is_some();
        if NO_REPO_ARGUMENT.contains(&tool.name) {
            assert!(!has, "{} should not take a repo", tool.name);
        } else {
            assert!(has, "{} takes no repo", tool.name);
            let required = schema["required"].as_array().cloned().unwrap_or_default();
            assert!(
                required.contains(&Value::from("repo")),
                "{} makes repo optional",
                tool.name
            );
        }
    }
}

#[test]
fn no_tool_takes_a_credential_as_an_argument() {
    let re = Regex::new("(?i)token|secret|password|api_?key|authorization|credential").unwrap();
    for tool in controllers::tools() {
        for key in tool.def.input_schema["properties"]
            .as_object()
            .unwrap()
            .keys()
        {
            assert!(!re.is_match(key), "{}.{key}", tool.name);
        }
    }
}

fn spec_named(module: &'static str, name: &'static str) -> ToolSpec {
    let mut spec = controllers::get("repo_read").unwrap().clone();
    spec.name = module;
    spec.def.name = name;
    spec
}

#[test]
fn the_registry_refuses_two_tools_with_one_name() {
    let problem = build_registry(vec![spec_named("same", "same"), spec_named("same", "same")])
        .err()
        .unwrap();
    assert_eq!(problem, "Duplicate tool name 'same'.");
}

#[test]
fn the_registry_refuses_a_module_whose_tool_is_named_otherwise() {
    let problem = build_registry(vec![spec_named("alpha", "beta")])
        .err()
        .unwrap();
    assert_eq!(problem, "alpha.rs defines the tool 'beta'.");
}

#[test]
fn tools_list_matches_the_published_surface_of_0_7_0() {
    let golden: Vec<Value> = serde_json::from_str(include_str!("fixtures/tools-list.json"))
        .expect("the fixture is JSON");
    let served: Vec<Value> = controllers::tools()
        .iter()
        .map(|t| serde_json::to_value(lxagents_github::routes::server::published(&t.def)).unwrap())
        .collect();
    assert_eq!(served.len(), golden.len());
    for mut want in golden {
        want.as_object_mut().unwrap().remove("execution");
        let got = served
            .iter()
            .find(|t| t["name"] == want["name"])
            .cloned()
            .unwrap_or_default();
        assert_eq!(got, want, "{} differs from 0.7.0", want["name"]);
    }
}
