# deptree

Et CLI-verktøy som leser output fra `mvn dependency:tree` (fra stdin, fil, eller ved å kjøre `mvn` selv), bygger opp en intern representasjon av avhengighetstreet, og lar deg søke etter en pakke og se hele grenen den bor på — eller dumpe hele treet.

Direkte avhengigheter vises i **grønt**, transitive avhengigheter i **gult**, og roten (prosjektet selv) i fet skrift.

## Installasjon

### Forhåndsbygde binærer

Last ned nyeste versjon fra [Releases](../../releases). Det finnes pakker for:

- **Linux (x86_64):** `.tar.gz` (rå binær) eller `.AppImage` (portabel, kjørbar direkte)
- **macOS (universal, x86_64 + arm64):** `.tar.gz`
- **Windows (x86_64):** `.zip`

Binærene er ikke signert/notarisert, så operativsystemet vil blokkere dem ved første kjøring med mindre du fjerner nedlastingsmarkeringen manuelt:

**macOS** — fjern quarantine-attributtet (`xattr`) etter utpakking, ellers vil Gatekeeper si at appen "cannot be opened because the developer cannot be verified":

```sh
tar -xzf deptree-*-macos-universal.tar.gz
xattr -d com.apple.quarantine deptree-*-macos-universal/deptree
```

**Windows** — fjern "Mark of the Web"-flagget (NTFS-ekvivalenten til `xattr`) etter utpakking, ellers vil SmartScreen blokkere kjøring:

```powershell
Expand-Archive deptree-*-x86_64-windows.zip
Unblock-File .\deptree-*-x86_64-windows\deptree.exe
```

(Alternativt: høyreklikk `.exe`-filen → Egenskaper → kryss av "Unblock" → OK.)

**Linux** — AppImage-filen må gjøres kjørbar først:

```sh
chmod +x deptree-*-x86_64.AppImage
./deptree-*-x86_64.AppImage guava
```

Hvis AppImage-kjøring feiler på grunn av manglende FUSE (vanlig i containere/eldre distroer), kjør den med `--appimage-extract-and-run`:

```sh
./deptree-*-x86_64.AppImage --appimage-extract-and-run guava
```

### Bygg selv fra kilde

```sh
cargo build --release
```

Binæren legges i `target/release/deptree`. Installer den lokalt i `~/.cargo/bin` med:

```sh
just install
# eller: cargo install --path .
```

## Bruk

### Finn hvor en pakke bor

```sh
mvn dependency:tree | deptree guava
```

eller la `deptree` kjøre `mvn` selv (skjer automatisk når stdin ikke er rørlagt og ingen `--file` er gitt):

```sh
deptree guava
```

Dette søker etter pakker som matcher `guava` som delstreng (case-insensitive) i `groupId:artifactId:packaging[:classifier]:version[:scope]`, og viser stien fra roten ned til hvert treff:

```
com.example:myapp:jar:1.0.0
  com.google.guava:guava-parent:pom:31.1:compile
    com.google.guava:guava:jar:31.1:compile
```

Flere treff skilles med en blank linje.

### Vis hele treet

Utelat pakkenavn for å dumpe hele avhengighetstreet:

```sh
deptree < tree.txt
```

### Les fra fil

```sh
deptree --file tree.txt guava
```

## Flagg

| Flagg | Betydning |
|---|---|
| `-f, --file <FILE>` | Les `dependency:tree`-output fra fil i stedet for stdin/mvn |
| `--mvn-arg <ARG>` | Ekstra argument til `mvn dependency:tree` (kan gis flere ganger, f.eks. `--mvn-arg -pl --mvn-arg mymodule`) |
| `--no-color` | Slå av farger (respekterer også standard `NO_COLOR`-miljøvariabelen automatisk) |
| `--include-scope <SCOPE>` | Vis kun avhengigheter med dette scopet. Kan gis flere ganger. Roten vises alltid |
| `--exclude-scope <SCOPE>` | Ekskluder avhengigheter med dette scopet, og hele undertreet deres. Kan gis flere ganger |
| `--no-transient` | Kun direkte avhengigheter — snarvei for `--search-depth 1` i søkemodus, `--depth 1` i dump-modus |
| `--search-depth <N>` | **Søkemodus:** hvor dypt i treet et treff kan ligge for å telle som treff (0 = kun roten, 1 = direkte avhengigheter, osv.) |
| `--depth <N>` | **Dump-modus:** hvor mange nivåer av treet som vises. **Søkemodus:** hvor mange nivåer av hvert treffs sti (fra roten) som vises — like trunkerte stier blant flere treff vises kun én gang |
| `--ignore <PAKKE>` | Ekskluder treff hvis pakken er en (transitiv) avhengighet av `<PAKKE>`. Kan gis flere ganger |
| `--module <NAVN>` | I et multi-modul-prosjekt: vis kun modul(er) (rot-koordinater) som matcher dette navnet. Kan gis flere ganger |
| `--json` | Skriv ut resultatet som JSON i stedet for farget tekst |

Gyldige verdier for `--include-scope`/`--exclude-scope` er `compile`, `provided`, `runtime`, `test` og `system` (case-insensitive). En ukjent scope gir en feilmelding og avslutter programmet før input leses:

```sh
$ deptree --exclude-scope bogus
Ukjent scope "bogus". Gyldige scopes: compile, provided, runtime, test, system
```

### Søkedybde vs. visningsdybde

`--search-depth` og `--depth` løser to forskjellige problemer i søkemodus:

- `--search-depth` avgjør **hvilke** noder som telles som treff. Et pakkenavn som finnes langt nede i treet, men dypere enn `--search-depth`, regnes ikke som treff.
- `--depth` avgjør **hvor mye** av hvert treffs sti som vises. Stien trunkeres til de øverste N nivåene fra roten, og hvis flere treff ender opp med identisk trunkert sti, vises den kun én gang.

Eksempel: søk etter `tomcat` finner tre treff i et dypt tre (selve tomcat-starteren og to av dens egne avhengigheter som også har "tomcat" i navnet). Med `--depth 1` vises kun den (trunkerte, dedupliserte) stien fram til første nivå, i stedet for tre nesten identiske blokker:

```sh
deptree --depth 1 tomcat
```

### JSON-output

**Søkemodus** (`deptree --json guava`):

```json
{
  "matches": [
    [
      { "depth": 0, "coordinate": "com.example:myapp:jar:1.0.0", "kind": "root" },
      { "depth": 1, "coordinate": "com.google.guava:guava:jar:31.1:compile", "kind": "direct" }
    ]
  ]
}
```

**Dump-modus** (`deptree --json`):

```json
{
  "tree": [
    {
      "coordinate": "com.example:myapp:jar:1.0.0",
      "kind": "root",
      "children": [ ... ]
    }
  ]
}
```

## Utvikling

```sh
just build     # cargo build
just test      # cargo test
just lint      # cargo clippy -D warnings
just fmt       # cargo fmt
just check     # fmt-check + lint + test
```
