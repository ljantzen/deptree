# Changelog

Alle vesentlige endringer i dette prosjektet dokumenteres her.

Formatet følger [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-11-02

### Lagt til

- Grunnleggende CLI: les `mvn dependency:tree`-output fra stdin, fil (`--file`), eller ved å kjøre `mvn dependency:tree` selv når stdin ikke er rørlagt.
- Søkemodus: gitt et pakkenavn, finn alle treff (substreng, case-insensitive) og vis stien fra roten ned til hvert treff.
- Dump-modus: uten pakkenavn vises hele avhengighetstreet.
- Fargelegging: roten i fet skrift, direkte avhengigheter i grønt, transitive i gult. Respekterer `NO_COLOR`-konvensjonen automatisk, og kan slås av eksplisitt med `--no-color`.
- Støtte for multi-modul-prosjekter: hver modul i `dependency:tree`-output behandles som et eget tre.
- `--include-scope` / `--exclude-scope`: filtrer avhengigheter (og undertreet deres) på Maven-scope. Ukjent scope gir en feilmelding (gyldige: `compile`, `provided`, `runtime`, `test`, `system`) før input leses.
- `--ignore <PAKKE>`: ekskluder treff som er en (transitiv) avhengighet av en gitt pakke.
- `--module <NAVN>`: i multi-modul-prosjekter, begrens til modul(er) som matcher et navn.
- `--search-depth` / `--depth`: skiller søkedybde (hvilke treff som telles) fra visningsdybde (hvor mye av hver sti som vises, med deduplisering av identiske trunkerte stier).
- `--no-transient`: snarvei for kun direkte avhengigheter.
- `--json`: strukturert JSON-output for både søk- og dump-modus.
- `justfile` med `build`, `test`, `lint`, `fmt`, `check` og `install`.
- GitHub Actions CI (`.github/workflows/ci.yml`): `fmt --check`, `clippy -D warnings` og `test` på Linux, macOS og Windows for hver push/PR.
- GitHub Actions Release (`.github/workflows/release.yml`): ved push av en `v*.*.*`-tag bygges og publiseres ferdige binærer til GitHub Releases for Linux x86_64 (`.tar.gz` og `.AppImage`), macOS universal x86_64+arm64 (`.tar.gz`), og Windows x86_64 (`.zip`).
- Dependabot (`.github/dependabot.yml`): ukentlige oppdateringer av både Cargo-avhengigheter og GitHub Actions.
