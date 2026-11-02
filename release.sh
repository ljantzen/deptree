#!/usr/bin/env bash
# Lag en ny release: oppdater versjon i Cargo.toml, flytt [Unreleased] i
# CHANGELOG.md til en datert seksjon, kjør sjekker, commit og tag.
# Tagging av formen vX.Y.Z trigger .github/workflows/release.yml, som bygger
# og publiserer binærer til GitHub Releases.
set -euo pipefail

usage() {
    echo "Bruk: $0 <versjon>" >&2
    echo "  <versjon>  Ny versjon, f.eks. 0.2.0 (uten ledende 'v')" >&2
    exit 1
}

[[ $# -eq 1 ]] || usage
VERSION="$1"

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Feil: versjon må være på formen x.y.z, fikk \"$VERSION\"" >&2
    exit 1
fi

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
    echo "Feil: arbeidstreet har usporede/endrede filer. Commit eller stash først." >&2
    exit 1
fi

# I et jj-colocated repo er git sin HEAD alltid løsrevet (det er jj sin
# normale modell, ikke en feiltilstand) - sjekk da ancestry mot "main" i
# stedet for grennavn.
if git symbolic-ref -q HEAD >/dev/null; then
    CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
    if [[ "$CURRENT_BRANCH" != "main" ]]; then
        echo "Feil: må være på 'main' (er på '$CURRENT_BRANCH')" >&2
        exit 1
    fi
else
    if ! git merge-base --is-ancestor main HEAD 2>/dev/null; then
        echo "Feil: HEAD er løsrevet og ikke en etterkommer av 'main'" >&2
        exit 1
    fi
fi

if git rev-parse "v$VERSION" >/dev/null 2>&1; then
    echo "Feil: tag v$VERSION finnes allerede" >&2
    exit 1
fi

if ! grep -q '^## \[Unreleased\]' CHANGELOG.md; then
    echo "Feil: CHANGELOG.md har ingen [Unreleased]-seksjon å gjøre til v$VERSION" >&2
    exit 1
fi

echo "==> Oppdaterer Cargo.toml til versjon $VERSION"
sed -i.bak -E "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
rm -f Cargo.toml.bak

echo "==> Oppdaterer Cargo.lock"
cargo check -q

echo "==> Oppdaterer CHANGELOG.md"
TODAY="$(date +%Y-%m-%d)"
awk -v version="$VERSION" -v today="$TODAY" '
    /^## \[Unreleased\]/ {
        print
        print ""
        print "## [" version "] - " today
        next
    }
    { print }
' CHANGELOG.md > CHANGELOG.md.tmp
mv CHANGELOG.md.tmp CHANGELOG.md

echo "==> Kjører sjekker (fmt, clippy, test)"
just check

echo "==> Committer"
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release v$VERSION"

echo "==> Tagger v$VERSION"
git tag -a "v$VERSION" -m "v$VERSION"

echo
read -r -p "Push commit og tag til origin nå? [y/N] " CONFIRM
if [[ "$CONFIRM" =~ ^[Yy]$ ]]; then
    git push origin HEAD:main
    git push origin "v$VERSION"
    echo "==> Pushet. GitHub Actions bygger og publiserer releasen nå."
else
    echo "==> Hoppet over push. Kjør manuelt når du er klar:"
    echo "    git push origin HEAD:main && git push origin v$VERSION"
fi
