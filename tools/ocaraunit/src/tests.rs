// Tests unitaires de la logique pure d'ocaraunit (découverte, exclusions,
// imports, cache, ligne de commande).

use std::path::{Path, PathBuf};

use crate::{
    cache::EntryName,
    cli::{parse_args, DEFAULT_TIMEOUT_S},
    config::is_excluded,
    discovery::{declares_main, extract_tests, TestEntry},
    imports::{parse_import, relative_to, ImportPath},
    process::strip_ansi,
};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn names(source: &str) -> Vec<String> {
    extract_tests(source).into_iter().map(|t| t.name).collect()
}

#[test]
fn discovery_free_functions_next_to_a_class() {
    let src = "class Helper {\n    public method fooTest(): int { return 0 }\n}\n\
               function barTest(): int {\n    return 0\n}\n";
    let tests = extract_tests(src);
    assert_eq!(tests, vec![
        TestEntry { class: Some("Helper".into()), name: "fooTest".into(), is_static: false },
        TestEntry { class: None, name: "barTest".into(), is_static: false },
    ]);
}

#[test]
fn discovery_tolerates_spaces_and_tabs() {
    let src = "class XTest extends UnitTest {\n    public  method aTest(): int { return 0 }\n\
               \tpublic\tstatic\tmethod bTest(): int { return 0 }\n}\n";
    let tests = extract_tests(src);
    assert_eq!(names(src), vec!["aTest", "bTest"]);
    assert!(tests[1].is_static);
}

#[test]
fn discovery_ignores_private_strings_and_comments() {
    let src = "class XTest {\n    private method hTest(): int { return 0 }\n\
               // public method cTest(): int\n    public method s(): string { return \"{\" }\n\
               public method zTest(): int { return 0 }\n}\n";
    assert_eq!(names(src), vec!["zTest"]);
}

#[test]
fn discovery_class_brace_on_next_line() {
    let src = "class XTest\n{\n    public method aTest(): int { return 0 }\n}\n";
    assert_eq!(names(src), vec!["aTest"]);
}

#[test]
fn discovery_main_and_nested_functions() {
    assert!(declares_main("function main(): int {\n    return 0\n}\n"));
    assert!(!declares_main("class A {\n    public method main(): int { return 0 }\n}\n"));
    // Méthode d'un module : jamais instanciée comme test
    assert!(names("module M {\n    public method aTest(): int { return 0 }\n}\n").is_empty());
}

#[test]
fn exclusions_by_path_component() {
    let root = Path::new("/p");
    let ex = vec!["src/".to_string()];
    assert!(is_excluded(Path::new("/p/src/A.oc"), root, &ex));
    assert!(is_excluded(Path::new("/p/lib/src/A.oc"), root, &ex));
    assert!(!is_excluded(Path::new("/p/mysrc/A.oc"), root, &ex));
    assert!(is_excluded(Path::new("/p/a/B.oc"), root, &["a/B.oc".to_string()]));
}

#[test]
fn import_lines() {
    assert_eq!(parse_import("import Helper from \"../lib/Helper\""), Some(ImportPath::From("../lib/Helper".into())));
    assert_eq!(parse_import("  import a.b.C as D"), Some(ImportPath::Dotted("a/b/C".into())));
    assert_eq!(parse_import("import ocara.UnitTest"), None);
    assert_eq!(parse_import("importer"), None);
}

#[test]
fn relative_paths_outside_root() {
    let rel = relative_to(Path::new("/a/lib/Helper"), Path::new("/a/proj"));
    assert_eq!(rel, PathBuf::from("../lib/Helper"));
    assert_eq!(relative_to(Path::new("/a/proj/x/Y"), Path::new("/a/proj")), PathBuf::from("x/Y"));
}

#[test]
fn cache_purge_selection() {
    let entry = EntryName::of(Path::new("/nonexistent/tests/FooTest.oc"));
    let keep = entry.path(Path::new("/c"), 0xabc).file_name().unwrap().to_string_lossy().to_string();
    let older = entry.path(Path::new("/c"), 0x123).file_name().unwrap().to_string_lossy().to_string();
    assert!(!entry.is_stale(&keep, &keep));
    assert!(entry.is_stale(&format!("{}.o", keep), &keep));
    assert!(entry.is_stale(&older, &keep));
    assert!(entry.is_stale(&format!("{}.o", older), &keep));
    assert!(entry.is_stale("FooTest-15deac051ca2d392", &keep));
    assert!(!entry.is_stale("BarTest-15deac051ca2d392", &keep));
    // Même stem, autre dossier : version d'un autre fichier, conservée
    assert!(!entry.is_stale("FooTest-1234-abcd", &keep));
}

#[test]
fn cli_coverage_does_not_swallow_tests_dir() {
    let o = parse_args(args(&["--coverage", "examples/tests"])).unwrap();
    assert!(o.coverage);
    assert_eq!(o.coverage_target, None);
    assert_eq!(o.target.as_deref(), Some("examples/tests"));
    let o = parse_args(args(&["--coverage=src", "--timeout", "5"])).unwrap();
    assert_eq!(o.coverage_target.as_deref(), Some("src"));
    assert_eq!(o.timeout_s, 5);
    assert_eq!(parse_args(args(&[])).unwrap().timeout_s, DEFAULT_TIMEOUT_S);
    // Deux positionnels : celui qui suit --coverage est le périmètre (CI)
    let o = parse_args(args(&["--coverage", "src/A.oc", "tests/ATest.oc"])).unwrap();
    assert_eq!(o.coverage_target.as_deref(), Some("src/A.oc"));
    assert_eq!(o.target.as_deref(), Some("tests/ATest.oc"));
    let o = parse_args(args(&["tests/ATest.oc", "--coverage", "src/A.oc"])).unwrap();
    assert_eq!(o.coverage_target.as_deref(), Some("src/A.oc"));
    assert_eq!(o.target.as_deref(), Some("tests/ATest.oc"));
}

#[test]
fn cli_rejects_unknown_and_extra_arguments() {
    assert!(parse_args(args(&["--bogus"])).is_err());
    assert!(parse_args(args(&["a", "b"])).is_err());
    assert!(parse_args(args(&["--timeout", "0"])).is_err());
    assert!(parse_args(args(&["--src"])).is_err());
}

#[test]
fn ansi_codes_stripped() {
    assert_eq!(strip_ansi("\x1b[31mMessage: x\x1b[0m"), "Message: x");
}

#[test]
fn coverage_ignores_case_and_underscores() {
    use crate::coverage::{compute_coverage, extract_all_function_names, normalize};
    let src = "class A {\n    public method add_bonus(): int { return 0 }\n    public method init(): void {}\n}\n\
               function main(): int {\n    return 0\n}\n";
    let funcs = extract_all_function_names(src);
    assert_eq!(funcs, vec![("add_bonus".to_string(), 2)]);
    let names = [normalize("addBonusTest")].into_iter().collect();
    assert_eq!(compute_coverage(Path::new("a.oc"), &funcs, &names).covered_funcs, 1);
}

// ── Couverture par ligne (lines.rs) ──

mod line_coverage {
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::cli::parse_args;
    use crate::lines::{to_json, FileLines, Hits, Report};

    fn lines(points: &[(usize, bool)]) -> BTreeMap<usize, bool> {
        points.iter().copied().collect()
    }

    #[test]
    fn consecutive_uncovered_lines_form_one_range() {
        let f = FileLines::of(Path::new("a.oc"), &lines(&[(2, true), (3, false), (5, false), (7, true), (9, false)]));
        assert_eq!(f.uncovered, vec![(3, 5), (9, 9)]);
        assert_eq!((f.covered, f.total), (2, 5));
        assert_eq!(f.percent(), 40.0);
    }

    #[test]
    fn a_line_hit_by_any_test_is_covered() {
        let root = Path::new("/projet");
        let mut hits = Hits::default();
        hits.add_report("0\t4\t/projet/a.oc\n1\t5\t/projet/a.oc\n", root);
        hits.add_report("1\t4\t/projet/a.oc\n0\t5\t/projet/a.oc\n", root);
        hits.add_points("6\t/projet/a.oc\n", root);
        let file = &hits.files[Path::new("/projet/a.oc")];
        assert_eq!(file, &lines(&[(4, true), (5, true), (6, false)]));
    }

    #[test]
    fn relative_report_paths_are_resolved_from_the_root() {
        let mut hits = Hits::default();
        hits.add_report("1\t1\tsrc/a.oc\n", Path::new("/projet"));
        assert!(hits.files.contains_key(Path::new("/projet/src/a.oc")));
    }

    #[test]
    fn json_report_lists_files_and_ranges() {
        let files = vec![FileLines::of(Path::new("/p/\"a\".oc"), &lines(&[(1, true), (2, false)]))];
        let json = to_json(&Report { pass: 3, fail: 0, errors: 1, files: &files, unanalyzed: &[] });
        assert!(json.contains("\"tests\":{\"pass\":3,\"fail\":0,\"errors\":1}"), "{}", json);
        assert!(json.contains("\"path\":\"/p/\\\"a\\\".oc\""), "{}", json);
        assert!(json.contains("\"uncovered\":[{\"start\":2,\"end\":2}]"), "{}", json);
        assert!(json.contains("\"percent\":50.00"), "{}", json);
        assert!(json.contains("\"points\":[[1,1],[2,0]]"), "{}", json);
    }

    #[test]
    fn line_coverage_exclude_and_json_options() {
        let opts = parse_args(["--line-coverage=src", "--exclude", "core", "--exclude=main.oc", "--json", "out.json", "tests"]
            .iter().map(|s| s.to_string())).expect("options valides");
        assert!(opts.line_coverage && !opts.coverage);
        assert_eq!(opts.coverage_target.as_deref(), Some("src"));
        assert_eq!(opts.excludes, vec!["core".to_string(), "main.oc".to_string()]);
        assert_eq!(opts.json.as_deref(), Some("out.json"));
        assert_eq!(opts.target.as_deref(), Some("tests"));
    }
}
