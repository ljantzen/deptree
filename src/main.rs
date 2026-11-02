use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Command, exit};

/// Les Maven dependency:tree-output og vis grenen en gitt pakke bor på.
#[derive(Parser)]
#[command(
    name = "deptree",
    about = "Finn og vis avhengighetsgrenen for en pakke i Maven dependency:tree-output",
    override_usage = "deptree [OPTIONS] [PACKAGE]"
)]
struct Cli {
    /// Pakke å søke etter (artifactId, "groupId:artifactId", eller del av koordinat).
    /// Hvis utelatt, vises hele avhengighetstreet.
    package: Option<String>,

    /// Les dependency:tree-output fra fil i stedet for stdin/mvn
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Ekstra argumenter som sendes til "mvn dependency:tree" (f.eks. -pl modul)
    #[arg(long = "mvn-arg")]
    mvn_args: Vec<String>,

    /// Slå av farger i output
    #[arg(long)]
    no_color: bool,

    /// Vis kun avhengigheter med dette scopet (f.eks. compile, runtime,
    /// provided, test). Kan gis flere ganger. Roten (prosjektet selv) vises
    /// alltid.
    #[arg(long)]
    include_scope: Vec<String>,

    /// Ekskluder avhengigheter med dette scopet (og undertreet deres).
    /// Kan gis flere ganger, f.eks. --exclude-scope test
    #[arg(long)]
    exclude_scope: Vec<String>,

    /// Kun direkte avhengigheter: i søkemodus tilsvarer det --search-depth 1,
    /// i dump-modus (uten pakkenavn) tilsvarer det --depth 1
    #[arg(long)]
    no_transient: bool,

    /// Søkemodus: hvor dypt i treet et treff kan ligge for å telle som treff
    /// (0 = kun roten, 1 = direkte avhengigheter, osv.). Har ingen effekt i
    /// dump-modus.
    #[arg(long)]
    search_depth: Option<usize>,

    /// Dump-modus (uten pakkenavn): hvor mange nivåer av treet som vises,
    /// regnet fra roten. Søkemodus: hvor mange nivåer av hvert treffs sti
    /// (fra roten) som vises; identiske trunkerte stier vises kun én gang.
    #[arg(long)]
    depth: Option<usize>,

    /// Skriv ut resultatet som JSON i stedet for farget tekst
    #[arg(long)]
    json: bool,

    /// Ekskluder treff hvis pakken er en avhengighet av <PAKKE> (kan gis flere ganger)
    #[arg(long)]
    ignore: Vec<String>,

    /// I et multi-modul-prosjekt: vis kun modul(er) (rot-koordinater) som
    /// matcher dette navnet. Kan gis flere ganger.
    #[arg(long)]
    module: Vec<String>,
}

/// En enkelt linje i treet: (dybde, koordinat). Dybde 0 = prosjektet selv,
/// dybde 1 = direkte avhengighet, dybde >= 2 = transitiv avhengighet.
type TreeLine = (usize, String);

/// Gyldige Maven-scopes slik de forekommer i dependency:tree-output.
const VALID_SCOPES: &[&str] = &["compile", "provided", "runtime", "test", "system"];

fn validate_scopes(scopes: &[String]) -> Result<(), String> {
    for scope in scopes {
        if !VALID_SCOPES.iter().any(|s| s.eq_ignore_ascii_case(scope)) {
            return Err(format!(
                "Ukjent scope \"{scope}\". Gyldige scopes: {}",
                VALID_SCOPES.join(", ")
            ));
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    if let Err(e) = validate_scopes(&cli.include_scope) {
        eprintln!("{e}");
        exit(1);
    }
    if let Err(e) = validate_scopes(&cli.exclude_scope) {
        eprintln!("{e}");
        exit(1);
    }

    let input = match read_input(&cli) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Feil ved lesing av input: {e}");
            exit(1);
        }
    };

    let mut forest = build_forest(&input);

    if !cli.module.is_empty() {
        forest.retain(|root| cli.module.iter().any(|m| coord_matches(&root.coord, m)));
    }

    filter_forest(
        &mut forest,
        &cli.include_scope,
        &cli.exclude_scope,
        &cli.ignore,
    );

    match &cli.package {
        Some(package) => {
            let search_depth = match (cli.no_transient, cli.search_depth) {
                (true, Some(d)) => Some(d.min(1)),
                (true, None) => Some(1),
                (false, d) => d,
            };

            let hits = find_hits(&forest, package, search_depth);
            let paths = truncate_and_dedup_hits(&hits, cli.depth);

            if cli.json {
                print_json_paths(&paths);
                if paths.is_empty() {
                    exit(1);
                }
                return;
            }

            if paths.is_empty() {
                eprintln!("Fant ingen pakke som matcher \"{package}\" i avhengighetstreet.");
                exit(1);
            }

            for (i, path) in paths.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print_branch(path);
            }
        }
        None => {
            let depth = match (cli.no_transient, cli.depth) {
                (true, Some(d)) => Some(d.min(1)),
                (true, None) => Some(1),
                (false, d) => d,
            };
            truncate_depth(&mut forest, depth);

            if cli.json {
                print_json_tree(&forest);
                if forest.is_empty() {
                    exit(1);
                }
                return;
            }

            if forest.is_empty() {
                eprintln!("Fant ikke noe gyldig avhengighetstre i input.");
                exit(1);
            }

            for (i, root) in forest.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print_node(root);
            }
        }
    }
}

#[derive(Serialize)]
struct JsonNode {
    depth: usize,
    coordinate: String,
    kind: &'static str,
}

fn node_kind(depth: usize) -> &'static str {
    match depth {
        0 => "root",
        1 => "direct",
        _ => "transitive",
    }
}

/// En node i hele avhengighetstreet.
#[derive(Clone)]
struct Node {
    depth: usize,
    coord: String,
    children: Vec<Node>,
}

/// Bygg et skog av tre(er) (ett per modul/rot) fra rå dependency:tree-output.
fn build_forest(input: &str) -> Vec<Node> {
    let lines: Vec<TreeLine> = input
        .lines()
        .filter_map(|l| parse_line(strip_info_prefix(l)))
        .collect();
    let mut iter = lines.into_iter().peekable();
    build_level(&mut iter, 0)
}

fn build_level(
    iter: &mut std::iter::Peekable<std::vec::IntoIter<TreeLine>>,
    depth: usize,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    while let Some(&(d, _)) = iter.peek() {
        if d < depth {
            break;
        }
        let (_, coord) = iter.next().unwrap();
        let children = build_level(iter, depth + 1);
        nodes.push(Node {
            depth,
            coord,
            children,
        });
    }
    nodes
}

/// Fjern noder (og undertreet deres) hvis scope ikke er tillatt (se
/// `scope_allowed`), eller som matcher et av de ignorerte pakkenavnene.
fn filter_forest(
    nodes: &mut Vec<Node>,
    include_scopes: &[String],
    exclude_scopes: &[String],
    ignore: &[String],
) {
    nodes.retain(|n| {
        scope_allowed(&n.coord, include_scopes, exclude_scopes)
            && !ignore.iter().any(|needle| coord_matches(&n.coord, needle))
    });

    for n in nodes.iter_mut() {
        filter_forest(&mut n.children, include_scopes, exclude_scopes, ignore);
    }
}

/// Kapp bort undertre dypere enn `max_depth` (absolutt dybde fra roten).
fn truncate_depth(nodes: &mut [Node], max_depth: Option<usize>) {
    for n in nodes.iter_mut() {
        if max_depth.is_some_and(|max| n.depth >= max) {
            n.children.clear();
        } else {
            truncate_depth(&mut n.children, max_depth);
        }
    }
}

fn print_node(node: &Node) {
    println!("{}", format_line(node.depth, &node.coord));
    for child in &node.children {
        print_node(child);
    }
}

/// Et treff i søkemodus: stien fra roten ned til treffet, inkludert treffet selv.
struct Hit {
    ancestors: Vec<TreeLine>,
}

/// Finn alle noder i (eventuelt flere) tre(er) som matcher `needle`.
/// `max_depth`, hvis satt, utelukker treff dypere enn dette fra roten.
fn find_hits(forest: &[Node], needle: &str, max_depth: Option<usize>) -> Vec<Hit> {
    let mut hits = Vec::new();
    let mut ancestors = Vec::new();
    for root in forest {
        search_node(root, &mut ancestors, needle, max_depth, &mut hits);
    }
    hits
}

fn search_node(
    node: &Node,
    ancestors: &mut Vec<TreeLine>,
    needle: &str,
    max_depth: Option<usize>,
    hits: &mut Vec<Hit>,
) {
    ancestors.push((node.depth, node.coord.clone()));

    let within_depth = max_depth.is_none_or(|max| node.depth <= max);
    if within_depth && coord_matches(&node.coord, needle) {
        hits.push(Hit {
            ancestors: ancestors.clone(),
        });
    }

    for child in &node.children {
        search_node(child, ancestors, needle, max_depth, hits);
    }

    ancestors.pop();
}

/// Trunker hvert treffs sti til oppføringer med dybde <= `depth` (hvis satt),
/// og dedupliser like trunkerte stier (bevarer første forekomst sin rekkefølge).
/// Uten `depth` returneres hver sti uendret.
fn truncate_and_dedup_hits(hits: &[Hit], depth: Option<usize>) -> Vec<Vec<TreeLine>> {
    let mut seen: Vec<Vec<TreeLine>> = Vec::new();

    for hit in hits {
        let path: Vec<TreeLine> = match depth {
            Some(max) => hit
                .ancestors
                .iter()
                .filter(|(d, _)| *d <= max)
                .cloned()
                .collect(),
            None => hit.ancestors.clone(),
        };

        if !seen.contains(&path) {
            seen.push(path);
        }
    }

    seen
}

fn print_branch(path: &[TreeLine]) {
    for (depth, coord) in path {
        println!("{}", format_line(*depth, coord));
    }
}

#[derive(Serialize)]
struct JsonTreeNode {
    coordinate: String,
    kind: &'static str,
    children: Vec<JsonTreeNode>,
}

#[derive(Serialize)]
struct JsonTreeOutput {
    tree: Vec<JsonTreeNode>,
}

fn to_json_tree(nodes: &[Node]) -> Vec<JsonTreeNode> {
    nodes
        .iter()
        .map(|n| JsonTreeNode {
            coordinate: n.coord.clone(),
            kind: node_kind(n.depth),
            children: to_json_tree(&n.children),
        })
        .collect()
}

fn print_json_tree(nodes: &[Node]) {
    let output = JsonTreeOutput {
        tree: to_json_tree(nodes),
    };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

#[derive(Serialize)]
struct JsonHitOutput {
    matches: Vec<Vec<JsonNode>>,
}

fn print_json_paths(paths: &[Vec<TreeLine>]) {
    let output = JsonHitOutput {
        matches: paths
            .iter()
            .map(|path| {
                path.iter()
                    .map(|(depth, coord)| JsonNode {
                        depth: *depth,
                        coordinate: coord.clone(),
                        kind: node_kind(*depth),
                    })
                    .collect()
            })
            .collect(),
    };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

/// Finn kildedata: fil hvis angitt, ellers stdin hvis den er rørlagt (piped),
/// ellers kjør "mvn dependency:tree" selv og fang opp output.
fn read_input(cli: &Cli) -> io::Result<String> {
    if let Some(path) = &cli.file {
        return std::fs::read_to_string(path);
    }

    if !io::stdin().is_terminal() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        return Ok(buf);
    }

    run_mvn(&cli.mvn_args)
}

fn run_mvn(extra_args: &[String]) -> io::Result<String> {
    let output = Command::new("mvn")
        .arg("dependency:tree")
        .args(extra_args)
        .output()?;

    if !output.status.success() {
        io::stderr().write_all_and_flush(&output.stderr)?;
        return Err(io::Error::other(format!(
            "mvn avsluttet med kode {:?}",
            output.status.code()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

trait WriteAllFlush {
    fn write_all_and_flush(&mut self, buf: &[u8]) -> io::Result<()>;
}

impl<W: io::Write> WriteAllFlush for W {
    fn write_all_and_flush(&mut self, buf: &[u8]) -> io::Result<()> {
        self.write_all(buf)?;
        self.flush()
    }
}

/// Fjern "[INFO] "-prefiks fra mvn-output, dersom det finnes.
fn strip_info_prefix(line: &str) -> &str {
    line.strip_prefix("[INFO] ").unwrap_or(line)
}

/// Tolk en enkelt linje til (dybde, koordinat). Returnerer None for linjer
/// som ikke er gyldige tre-linjer (tomme linjer, mvn-logglinjer, osv).
fn parse_line(line: &str) -> Option<TreeLine> {
    let mut s = line;
    let mut depth = 0usize;

    loop {
        if s.len() < 3 {
            break;
        }
        let chunk = &s[0..3];
        match chunk {
            "|  " | "   " => {
                s = &s[3..];
                depth += 1;
            }
            "+- " | "\\- " => {
                s = &s[3..];
                depth += 1;
                break;
            }
            _ => break,
        }
    }

    let coord = s.trim();
    if coord.is_empty() || coord.matches(':').count() < 2 {
        return None;
    }
    if !coord
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ":._-".contains(c))
    {
        return None;
    }

    Some((depth, coord.to_string()))
}

fn coord_matches(coord: &str, needle: &str) -> bool {
    coord.to_lowercase().contains(&needle.to_lowercase())
}

/// Hent ut scope-feltet fra en koordinat, hvis det finnes.
/// Format: groupId:artifactId:packaging[:classifier]:version[:scope]
/// Prosjekt-/rot-linjer har ikke scope (4 felt) og returnerer None.
fn coord_scope(coord: &str) -> Option<&str> {
    let parts: Vec<&str> = coord.split(':').collect();
    if parts.len() >= 5 {
        parts.last().copied()
    } else {
        None
    }
}

/// Avgjør om en koordinat sitt scope er tillatt: den må stå i
/// `include_scopes` (hvis den lista ikke er tom), og må ikke stå i
/// `exclude_scopes`. Rot-/prosjekt-linjer har ikke scope og er alltid tillatt.
fn scope_allowed(coord: &str, include_scopes: &[String], exclude_scopes: &[String]) -> bool {
    match coord_scope(coord) {
        Some(scope) => {
            let included = include_scopes.is_empty()
                || include_scopes.iter().any(|s| s.eq_ignore_ascii_case(scope));
            let excluded = exclude_scopes.iter().any(|s| s.eq_ignore_ascii_case(scope));
            included && !excluded
        }
        None => true,
    }
}

/// Formater en koordinat med innrykk og farge basert på dybde: fet for
/// roten, grønn for direkte avhengigheter, gul for transitive.
fn format_line(depth: usize, coord: &str) -> colored::ColoredString {
    let indent = "  ".repeat(depth);
    let text = format!("{indent}{coord}");
    match depth {
        0 => text.bold(),
        1 => text.green(),
        _ => text.yellow(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root_line() {
        assert_eq!(
            parse_line("com.example:myapp:jar:1.0.0"),
            Some((0, "com.example:myapp:jar:1.0.0".to_string()))
        );
    }

    #[test]
    fn parses_direct_dependency() {
        assert_eq!(
            parse_line("+- org.springframework:spring-core:jar:5.3.9:compile"),
            Some((
                1,
                "org.springframework:spring-core:jar:5.3.9:compile".to_string()
            ))
        );
    }

    #[test]
    fn parses_transitive_dependency() {
        assert_eq!(
            parse_line("|  \\- org.springframework:spring-jcl:jar:5.3.9:compile"),
            Some((
                2,
                "org.springframework:spring-jcl:jar:5.3.9:compile".to_string()
            ))
        );
    }

    #[test]
    fn rejects_noise_lines() {
        assert_eq!(parse_line("Scanning for projects..."), None);
        assert_eq!(
            parse_line("------------------------------------------------------------------------"),
            None
        );
        assert_eq!(parse_line(""), None);
    }

    const SAMPLE: &str = r"com.example:myapp:jar:1.0.0
+- org.springframework:spring-core:jar:5.3.9:compile
|  \- org.springframework:spring-jcl:jar:5.3.9:compile
\- junit:junit:jar:4.13.2:test
   \- org.hamcrest:hamcrest-core:jar:1.3:test
";

    #[test]
    fn finds_branch_for_transitive_dependency() {
        let forest = build_forest(SAMPLE);
        let hits = find_hits(&forest, "hamcrest-core", None);
        assert_eq!(hits.len(), 1);
        let path = &hits[0].ancestors;
        assert_eq!(path.len(), 3);
        assert_eq!(path[0].0, 0);
        assert_eq!(path[1].0, 1);
        assert_eq!(path[2].0, 2);
        assert!(path[2].1.contains("hamcrest-core"));
    }

    #[test]
    fn finds_direct_dependency_branch() {
        let forest = build_forest(SAMPLE);
        let hits = find_hits(&forest, "spring-core", None);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].ancestors.len(), 2);
    }

    #[test]
    fn no_match_returns_empty() {
        let forest = build_forest(SAMPLE);
        assert!(find_hits(&forest, "nonexistent-package", None).is_empty());
    }

    #[test]
    fn multiple_modules_are_treated_as_separate_trees() {
        let input = "com.example:module-a:jar:1.0.0\n\
\\- com.google.guava:guava:jar:31.1:compile\n\
com.example:module-b:jar:1.0.0\n\
\\- com.google.guava:guava:jar:31.1:compile\n";
        let forest = build_forest(input);
        let hits = find_hits(&forest, "guava", None);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].ancestors[0].1.contains("module-a"));
        assert!(hits[1].ancestors[0].1.contains("module-b"));
    }

    #[test]
    fn excludes_test_scope_subtree() {
        let mut forest = build_forest(SAMPLE);
        filter_forest(&mut forest, &[], &["test".to_string()], &[]);

        // hamcrest-core bor under junit, som er scope "test" -> hele grenen skal filtreres bort.
        assert!(find_hits(&forest, "hamcrest-core", None).is_empty());
        assert!(find_hits(&forest, "junit", None).is_empty());

        // compile-scope avhengigheter skal ikke påvirkes.
        let hits = find_hits(&forest, "spring-core", None);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn ignore_excludes_matches_inside_ignored_subtree() {
        let mut forest = build_forest(SAMPLE);
        filter_forest(&mut forest, &[], &[], &["spring-core".to_string()]);

        // spring-jcl bor under spring-core, som nå er filtrert bort.
        assert!(find_hits(&forest, "spring-jcl", None).is_empty());

        // junit er ikke påvirket.
        let hits = find_hits(&forest, "junit", None);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn depth_excludes_deep_matches() {
        let forest = build_forest(SAMPLE);
        assert!(find_hits(&forest, "spring-jcl", Some(1)).is_empty());
        assert_eq!(find_hits(&forest, "spring-jcl", Some(2)).len(), 1);
        assert_eq!(find_hits(&forest, "spring-core", Some(1)).len(), 1);
    }

    #[test]
    fn truncate_and_dedup_hits_collapses_identical_truncated_paths() {
        // "tomcat" matcher to noder i samme gren: en direkte avhengighet og
        // et av dens egne barn. Med --depth 1 skal begge trunkeres til den
        // samme stien (rot + direkte avhengighet) og vises kun én gang.
        let input = r"root:app:jar:1.0
+- a:tomcat-starter:jar:1.0:compile
|  \- a:tomcat-core:jar:1.0:compile
\- a:other:jar:1.0:compile
";
        let forest = build_forest(input);
        let hits = find_hits(&forest, "tomcat", None);
        assert_eq!(hits.len(), 2);

        let paths = truncate_and_dedup_hits(&hits, Some(1));
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 2);
        assert_eq!(paths[0][1].1, "a:tomcat-starter:jar:1.0:compile");
    }

    #[test]
    fn truncate_and_dedup_hits_without_depth_keeps_full_paths() {
        let forest = build_forest(SAMPLE);
        let hits = find_hits(&forest, "hamcrest-core", None);
        let paths = truncate_and_dedup_hits(&hits, None);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 3);
    }

    fn coord_names(nodes: &[Node]) -> Vec<&str> {
        nodes.iter().map(|n| n.coord.as_str()).collect()
    }

    #[test]
    fn build_forest_preserves_structure() {
        let forest = build_forest(SAMPLE);
        assert_eq!(forest.len(), 1);
        let root = &forest[0];
        assert_eq!(root.depth, 0);
        assert_eq!(root.children.len(), 2);
        assert_eq!(
            coord_names(&root.children)[0],
            "org.springframework:spring-core:jar:5.3.9:compile"
        );
        assert_eq!(root.children[0].children.len(), 1);
        assert_eq!(
            coord_names(&root.children)[1],
            "junit:junit:jar:4.13.2:test"
        );
    }

    #[test]
    fn filter_forest_removes_excluded_scope_subtree() {
        let mut forest = build_forest(SAMPLE);
        filter_forest(&mut forest, &[], &["test".to_string()], &[]);
        assert_eq!(
            coord_names(&forest[0].children),
            vec!["org.springframework:spring-core:jar:5.3.9:compile"]
        );
    }

    #[test]
    fn validate_scopes_accepts_known_scopes_case_insensitively() {
        assert!(validate_scopes(&["compile".to_string(), "TEST".to_string()]).is_ok());
        assert!(validate_scopes(&[]).is_ok());
    }

    #[test]
    fn validate_scopes_rejects_unknown_scope() {
        let err = validate_scopes(&["bogus".to_string()]).unwrap_err();
        assert!(err.contains("bogus"));
        assert!(err.contains("compile"));
    }

    #[test]
    fn module_filter_keeps_only_matching_roots() {
        let input = "com.example:module-a:jar:1.0.0\n\
\\- com.google.guava:guava:jar:31.1:compile\n\
com.example:module-b:jar:1.0.0\n\
\\- com.google.guava:guava:jar:31.1:compile\n";
        let mut forest = build_forest(input);
        forest.retain(|root| coord_matches(&root.coord, "module-b"));
        assert_eq!(forest.len(), 1);
        assert!(forest[0].coord.contains("module-b"));
    }

    #[test]
    fn filter_forest_include_scope_keeps_only_matching() {
        let mut forest = build_forest(SAMPLE);
        filter_forest(&mut forest, &["test".to_string()], &[], &[]);
        // Kun "test"-scope avhengigheter skal bli igjen; roten holdes alltid.
        assert_eq!(
            coord_names(&forest[0].children),
            vec!["junit:junit:jar:4.13.2:test"]
        );
    }

    #[test]
    fn filter_forest_removes_ignored_subtree() {
        let mut forest = build_forest(SAMPLE);
        filter_forest(&mut forest, &[], &[], &["spring-context".to_string()]);
        assert_eq!(
            coord_names(&forest[0].children),
            vec![
                "org.springframework:spring-core:jar:5.3.9:compile",
                "junit:junit:jar:4.13.2:test"
            ]
        );
    }

    #[test]
    fn truncate_depth_one_clears_grandchildren() {
        let mut forest = build_forest(SAMPLE);
        truncate_depth(&mut forest, Some(1));
        for child in &forest[0].children {
            assert!(child.children.is_empty());
        }
    }

    #[test]
    fn truncate_depth_zero_keeps_only_root() {
        let mut forest = build_forest(SAMPLE);
        truncate_depth(&mut forest, Some(0));
        assert!(forest[0].children.is_empty());
    }
}
