#!/usr/bin/env bash
set -euo pipefail

root=${1:?usage: demo-repo.sh <empty directory>}
mkdir -p "$root"
cd "$root"
git -c init.defaultBranch=main init -q --bare origin.git
git -c init.defaultBranch=main init -q aurora-notes
git -c init.defaultBranch=main init -q aurora-site
cd aurora-notes
git remote add origin ../origin.git

day=0
as() {
  case $1 in
    alex) name="Alex Martin" ;;
    sam) name="Sam Rivera" ;;
    jordan) name="Jordan Lee" ;;
  esac
  export GIT_AUTHOR_NAME=$name GIT_AUTHOR_EMAIL="$1@example.com" GIT_COMMITTER_NAME=$name GIT_COMMITTER_EMAIL="$1@example.com"
  day=$((day + 1))
  local stamp="2026-09-$(printf %02d $(( day % 28 + 1 )))T$(printf %02d $(( 9 + day % 8 ))):$(printf %02d $(( day * 7 % 60 ))):00+0200"
  export GIT_AUTHOR_DATE=$stamp GIT_COMMITTER_DATE=$stamp
}

commit() {
  local who=$1 file=$2 msg=$3
  as "$who"
  mkdir -p "$(dirname "$file")"
  {
    printf '\n// %s\n' "$msg"
    printf 'export function step%s(input: string): string {\n' "$day"
    printf '  return input.trim();\n'
    printf '}\n'
  } >> "$file"
  git add "$file"
  git -c commit.gpgsign=false commit -qm "$msg"
}

merge() {
  as "$1"
  git -c commit.gpgsign=false merge -q --no-ff "$2" -m "Merge branch '$2'"
}

tag() {
  git -c tag.gpgsign=false tag -a "$1" -m "$2"
}

commit alex package.json "chore: project skeleton"
commit alex src/notes.ts "feat: note model and storage"
commit sam README.md "docs: getting started"
tag v0.1.0 "first preview"
git checkout -qb feature/search
commit jordan src/search.ts "feat(search): full-text index"
commit jordan src/search.ts "feat(search): highlight matches"
git checkout -q main
commit sam src/notes.ts "fix: keep note order after reload"
git checkout -qb feature/sync
commit alex src/sync.ts "feat(sync): conflict-free merge of edits"
git checkout -q main
merge sam feature/search
commit sam test/search.test.ts "test: search ranking"
git checkout -q feature/sync
commit alex src/sync.ts "feat(sync): offline queue"
git checkout -q main
commit jordan src/notes.ts "perf: lazy-load attachments"
tag v0.2.0 "search"
git checkout -qb fix/export-crlf
commit sam src/export.ts "fix(export): CRLF line endings on Windows"
git checkout -q main
merge sam fix/export-crlf
git checkout -q feature/sync
commit alex src/sync.ts "feat(sync): retry with backoff"
git checkout -q main
commit jordan src/theme.ts "feat: dark theme"
git checkout -qb feature/tags
commit jordan src/tags.ts "feat(tags): tag notes"
commit sam src/tags.ts "feat(tags): filter by tag"
git checkout -q main
commit alex package.json "chore: update dependencies"
git push -q origin main feature/sync feature/tags v0.1.0 v0.2.0
git checkout -q feature/sync
commit alex src/sync.ts "wip(sync): progress indicator"
git checkout -q main
git branch -q -D feature/search fix/export-crlf

printf '\nexport const version = "0.3.0-dev";\n' >> src/notes.ts
git add src/notes.ts
perl -pi -e 's/\Qreturn input.trim();\E/return input.trim().toLowerCase();/' src/search.ts
printf '# Roadmap\n\n- sync across devices\n- tags\n' > ROADMAP.md

cd ../aurora-site
commit sam index.html "feat: landing page"
