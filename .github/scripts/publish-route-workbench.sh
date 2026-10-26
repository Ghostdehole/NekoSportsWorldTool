#!/usr/bin/env bash
# Publish only the verified standalone planner package; never build the service client.
set -euo pipefail

REPO='Ghostdehole/NekoSportsWorldTool'
TAG='route-workbench-v0.1.0'
RELEASE_TITLE='Ghostdehole 路线工作台 v0.1.0'
SOURCE_SHA='3dce0c4493a74874903124f90eb19e9cfe1675c3'
SOURCE_TREE='f7e479ef205ef0a653a2d9150adc3a98c7b04529'
BUILD_SHA='6ba4d62079855b25bff025738c72418228a71af5'
RUN_ID='40298019430'
ARTIFACT_ID='12639890121'
ZIP_NAME='Neko-Route-Workbench-v0.1.0-Windows-x64.zip'
ZIP_SHA='2b36954171020b424d4527dfc5796e8511ff83769ae0a8c35a052b5f27bb5977'
EXE_SHA='258148fc9effa34981182c99f38a8264a41d6ebc41036797ee7ee7002e96eb77'
export REPO TAG RELEASE_TITLE SOURCE_SHA SOURCE_TREE BUILD_SHA ZIP_NAME ZIP_SHA EXE_SHA

[[ "${GITHUB_REPOSITORY:-}" == "$REPO" ]] || { echo 'Unexpected repository'; exit 1; }
NOTES="$GITHUB_WORKSPACE/docs/releases/$TAG.md"
export NOTES
TASK_DIR=$(mktemp -d "$RUNNER_TEMP/neko-publish.XXXXXX")
trap 'rm -rf "$TASK_DIR"' EXIT
cd "$TASK_DIR"

# A failed read is not equivalent to a missing resource.
fetch_optional() {
  local endpoint=$1 output=$2
  if gh api "$endpoint" > "$output" 2> request-error.txt; then
    return 0
  fi
  if [[ "$(cat request-error.txt)" == *'HTTP 404'* ]]; then
    printf 'null\n' > "$output"
  else
    cat request-error.txt >&2
    return 1
  fi
}

# REST /releases/tags only finds published releases; GraphQL also resolves drafts.
find_release() {
  local output=$1 release_id
  release_id=$(gh api graphql \
    -f query='query($owner:String!,$name:String!,$tag:String!){repository(owner:$owner,name:$name){release(tagName:$tag){databaseId}}}' \
    -f owner="${REPO%%/*}" -f name="${REPO#*/}" -f tag="$TAG" \
    --jq '.data.repository.release.databaseId // empty')
  if [[ -n "$release_id" ]]; then
    gh api "repos/$REPO/releases/$release_id" > "$output"
  else
    printf 'null\n' > "$output"
  fi
}

gh api "repos/$REPO/actions/runs/$RUN_ID" > run.json
gh api "repos/$REPO/actions/artifacts/$ARTIFACT_ID" > artifact.json
gh api "repos/$REPO/git/commits/$SOURCE_SHA" > source.json
fetch_optional "repos/$REPO/git/ref/tags/$TAG" tag.json
find_release release.json
python3 - <<'PY'
import json, os
read = lambda name: json.load(open(name, encoding='utf-8'))
run, artifact, source, tag = map(read, ['run.json', 'artifact.json', 'source.json', 'tag.json'])
assert run['status'] == 'completed' and run['conclusion'] == 'success', 'Build did not succeed'
assert run['head_sha'] == os.environ['BUILD_SHA'], 'Unexpected build source'
assert source['sha'] == os.environ['SOURCE_SHA'] and source['tree']['sha'] == os.environ['SOURCE_TREE'], 'Source snapshot mismatch'
assert artifact['name'] == 'Neko-Route-Workbench-Windows-x64' and not artifact['expired'], 'Artifact unavailable'
assert artifact['workflow_run']['head_sha'] == os.environ['BUILD_SHA'], 'Artifact source mismatch'
assert artifact['digest'] == 'sha256:' + os.environ['ZIP_SHA'], 'Artifact digest mismatch'
if tag:
    assert tag['object']['type'] == 'commit' and tag['object']['sha'] == os.environ['SOURCE_SHA'], 'Existing tag points elsewhere; refusing overwrite'
release = read('release.json')
if release:
    assert release['tag_name'] == os.environ['TAG'], 'Existing release tag mismatch'
    assert release['target_commitish'] == os.environ['SOURCE_SHA'], 'Existing release source mismatch'
    assert not release['prerelease'], 'Unexpected prerelease'
    assert release['name'] == os.environ['RELEASE_TITLE'], 'Existing release title mismatch'
    assert (release['body'] or '').strip() == open(os.environ['NOTES'], encoding='utf-8').read().strip(), 'Existing release notes differ'
    names = [a['name'] for a in release['assets']]
    assert len(names) == len(set(names)), 'Duplicate release assets'
    assert set(names) <= {os.environ['ZIP_NAME'], 'SHA256SUMS.txt'}, 'Unexpected assets; refusing to modify release'
print('Verified build, source snapshot, artifact metadata and release availability.')
PY

gh api "repos/$REPO/actions/artifacts/$ARTIFACT_ID/zip" > "$ZIP_NAME"
python3 - <<'PY'
import hashlib, os, struct, zipfile
p = os.environ['ZIP_NAME']
assert hashlib.sha256(open(p, 'rb').read()).hexdigest() == os.environ['ZIP_SHA'], 'Downloaded ZIP checksum mismatch'
with zipfile.ZipFile(p) as z:
    required = {'neko-route-workbench.exe', 'LICENSE', 'README.md', 'FORK_REVIEW.md', 'ROUTE_WORKBENCH_TESTS.md', 'SHA256.txt', 'examples/demo-route.json', 'examples/demo-network.osm', 'licenses/Noto-Sans-SC-OFL.txt', 'licenses/Noto-Sans-SC-source.md'}
    assert required == set(z.namelist()), 'Unexpected package contents'
    exe = z.read('neko-route-workbench.exe')
    assert hashlib.sha256(exe).hexdigest() == os.environ['EXE_SHA'], 'Executable checksum mismatch'
    offset = struct.unpack_from('<I', exe, 0x3c)[0]
    assert exe[:2] == b'MZ' and exe[offset:offset + 4] == b'PE\0\0'
    assert struct.unpack_from('<H', exe, offset + 4)[0] == 0x8664, 'Executable is not Windows x64'
    assert os.environ['EXE_SHA'] in z.read('SHA256.txt').decode('utf-8-sig').lower(), 'Bundled checksum mismatch'
print('Verified archive, Windows x64 executable, documentation and licenses.')
PY
printf '%s  %s\n' "$ZIP_SHA" "$ZIP_NAME" > SHA256SUMS.txt

if [[ "$(cat release.json)" == 'null' ]]; then
  gh release create "$TAG" --repo "$REPO" --target "$SOURCE_SHA" \
    --title "$RELEASE_TITLE" --notes-file "$NOTES" --draft
fi

# Resume a partial draft without clobbering existing assets or replacing a published release.
find_release release.json
for asset in "$ZIP_NAME" SHA256SUMS.txt; do
  if python3 - "$asset" <<'PY'
import json, sys
assets = json.load(open('release.json'))['assets']
sys.exit(0 if any(a['name'] == sys.argv[1] for a in assets) else 1)
PY
  then
    mkdir -p existing
    gh release download "$TAG" --repo "$REPO" --pattern "$asset" --dir existing
    cmp -- "$asset" "existing/$asset"
  else
    python3 - <<'PY'
import json
assert json.load(open('release.json'))['draft'], 'Published release lacks expected asset; refusing modification'
PY
    gh release upload "$TAG" "$asset" --repo "$REPO"
  fi
done

mkdir verify
gh release download "$TAG" --repo "$REPO" --pattern "$ZIP_NAME" --pattern SHA256SUMS.txt --dir verify
cmp -- "$ZIP_NAME" "verify/$ZIP_NAME"
cmp -- SHA256SUMS.txt verify/SHA256SUMS.txt
if python3 - <<'PY'
import json, sys
sys.exit(0 if json.load(open('release.json'))['draft'] else 1)
PY
then
  gh release edit "$TAG" --repo "$REPO" --draft=false --latest
fi
gh api "repos/$REPO/releases/tags/$TAG" > published.json
python3 - <<'PY'
import json, os
r = json.load(open('published.json'))
assert not r['draft'] and not r['prerelease'], 'Release is not public'
assert {os.environ['ZIP_NAME'], 'SHA256SUMS.txt'} <= {a['name'] for a in r['assets']}
print('Published:', r['html_url'])
with open(os.environ['GITHUB_STEP_SUMMARY'], 'a', encoding='utf-8') as f:
    f.write(f"Published [{r['name']}]({r['html_url']}) with verified Windows x64 ZIP and SHA256SUMS.txt.\n")
PY
