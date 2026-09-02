#!/usr/bin/env bash
# Shared cache ownership. Incomplete leases are protected; recovery is explicit CAS.
cargo_cache_lock_path() {
  local candidate="$1" parent name
  parent="$(dirname "$candidate")"; name="$(basename "$candidate")"
  [[ "$name" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || return 1
  case "$parent" in
    "${REPO_ROOT}/target-agent/pools") printf '%s/target-agent/.locks/pool-%s.lock\n' "$REPO_ROOT" "$name" ;;
    "${REPO_ROOT}/target-agent/agents") printf '%s/target-agent/.locks/agent-%s.lock\n' "$REPO_ROOT" "$name" ;;
    *) return 1 ;;
  esac
}
cargo_cache_lock_is_active() { [[ -e "$1" || -L "$1" ]]; }
cargo_cache_candidate_is_locked() { local lock; lock="$(cargo_cache_lock_path "$1")" || return 0; cargo_cache_lock_is_active "$lock"; }
cargo_cache_any_live_lock() {
  local lock
  for lock in "${REPO_ROOT}"/target-agent/.locks/*.lock; do
    [[ -e "$lock" || -L "$lock" ]] && return 0
  done
  return 1
}
cargo_cache_candidate_is_pinned() { [[ "$1" == "${REPO_ROOT}/target-agent/pools/${SCRIPT_KIT_AGENT_PINNED_POOL:-agent-debug}" ]]; }
cargo_cache_lease() { bash "${BASH_SOURCE[0]}" "$@"; }
cargo_cache_remove_candidate() {
  local candidate="$1" allow_pinned="${2:-0}" lock generation status=0
  # Explicit human recovery only; this function is never admission/agent GC.
  if [[ "${SCRIPT_KIT_NONINTERACTIVE:-1}" != "0" || "${SCRIPT_KIT_ALLOW_CARGO_CACHE_RECOVERY:-0}" != "1" ]]; then
    echo 'CARGO_CACHE refused: explicit authorized human recovery required' >&2; return 78
  fi
  [[ -d "$candidate" && ! -L "$candidate" ]] || return 1
  cargo_cache_candidate_is_pinned "$candidate" && [[ "$allow_pinned" != "1" ]] && return 1
  lock="$(cargo_cache_lock_path "$candidate")" || return 1
  generation="$(python3 -c 'import uuid; print(uuid.uuid4())')"
  cargo_cache_lease acquire "$lock" "$$" "$generation" 1000 >/dev/null || return 1
  rm -rf -- "$candidate" || status=1
  cargo_cache_lease release "$lock" "$$" "$generation" >/dev/null || status=1
  return "$status"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  exec python3 - "$@" <<'PY'
import hashlib, json, os, pathlib, shutil, stat, subprocess, sys, time, uuid

def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'))
def start(pid):
    if type(pid) is not int or pid <= 0:
        raise RuntimeError('owner_identity_invalid')
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return None
    except PermissionError:
        raise RuntimeError('owner_observation_unknown')
    out = subprocess.run(['ps', '-p', str(pid), '-o', 'lstart='], capture_output=True, text=True, timeout=1, env={**os.environ, 'LC_ALL':'C'})
    if out.returncode or not out.stdout.strip():
        raise RuntimeError('owner_start_unknown')
    return out.stdout.strip()
def child_identity(child):
    if (not isinstance(child, dict)
        or any(type(child.get(key)) is not int or child[key] <= 0 for key in ['pid', 'supervisorPid', 'processGroupId'])
        or child['processGroupId'] != child['pid']
        or any(not isinstance(child.get(key), str) or not child[key] for key in ['processStartTime', 'supervisorStartTime', 'processInstanceId', 'sessionGeneration'])):
        raise RuntimeError('lease_child_identity_invalid')
def observe_children(value):
    if value.get('pendingChild', False):
        raise RuntimeError('lease_child_identity_unknown')
    observations = []
    for child in value['children']:
        child_identity(child)
        observations += [{'pid':child['pid'], 'expected':child['processStartTime'], 'observed':start(child['pid'])},
                         {'pid':child['supervisorPid'], 'expected':child['supervisorStartTime'], 'observed':start(child['supervisorPid'])}]
    if value['children']:
        peers = subprocess.run(['ps', '-axo', 'pid=,pgid='], capture_output=True, text=True, timeout=1)
        if peers.returncode or not peers.stdout.strip():
            raise RuntimeError('group_observation_unknown')
        groups = set()
        for line in peers.stdout.splitlines():
            fields = line.split()
            if len(fields) != 2 or not all(field.isdecimal() for field in fields):
                raise RuntimeError('group_observation_unknown')
            groups.add(int(fields[1]))
        observations += [{'group':child['processGroupId'], 'observed':'present' if child['processGroupId'] in groups else None} for child in value['children']]
    return observations
def safe(path):
    for part in [path, *path.parents]:
        if part.is_symlink():
            raise RuntimeError('lease_path_symlink')
def snapshot(path):
    safe(path)
    info = path.stat()
    file = path / 'lease.json'
    fd = os.open(file, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        data = os.read(fd, 1024 * 1024)
        value = json.loads(data)
    finally:
        os.close(fd)
    if (value.get('schemaVersion') != 2 or value.get('directoryDevice') != info.st_dev
        or value.get('directoryInode') != info.st_ino or not value.get('processStartTime')
        or type(value.get('pid')) is not int or value['pid'] <= 0 or not isinstance(value.get('children'), list)
        or type(value.get('pendingChild', False)) is not bool):
        raise RuntimeError('lease_identity_invalid')
    return value, hashlib.sha256(data).hexdigest()
def write(path, value):
    tmp = path / ('lease.tmp-' + str(uuid.uuid4()))
    with tmp.open('x') as file:
        file.write(canonical(value) + '\n'); file.flush(); os.fsync(file.fileno())
    tmp.replace(path / 'lease.json')

def main():
    op, raw = sys.argv[1:3]
    path = pathlib.Path(raw).absolute()
    safe(path)
    if path.parent.name != '.locks' or not path.name.endswith('.lock'):
        raise RuntimeError('lease_scope_invalid')
    if op == 'diagnose':
        if not path.exists():
            return {'state':'absent', 'path':str(path)}
        try:
            value, digest = snapshot(path)
            owner = start(value['pid'])
            observations = [{'pid': value['pid'], 'expected':value['processStartTime'], 'observed':owner}]
            observations += observe_children(value)
            return {'state':'recoverable' if all(v['observed'] is None for v in observations) else 'protected',
                    'path':str(path), 'lease':value, 'recordSha256':digest, 'observations':observations}
        except Exception as error:
            return {'state':'protected', 'path':str(path), 'reasonCode':str(error)}
    if op == 'recover':
        expected = json.loads(sys.argv[3])
        actual = main_diagnose(path)
        if actual['state'] != 'recoverable' or any(actual.get(k) != expected.get(k) for k in ['recordSha256','lease']):
            raise RuntimeError('lease_recovery_compare_failed')
        current, digest = snapshot(path)
        if current != actual['lease'] or digest != actual['recordSha256']:
            raise RuntimeError('lease_recovery_identity_changed')
        confirmed = main_diagnose(path)
        if confirmed['state'] != 'recoverable' or any(confirmed.get(k) != actual.get(k) for k in ['recordSha256', 'lease']):
            raise RuntimeError('lease_recovery_observation_changed')
        quarantine = path.with_name(path.name + '.recovered-' + str(uuid.uuid4()))
        path.rename(quarantine)
        # Preserve evidence; releasing the exact name, not erasing the old receipt.
        return {'recovered':True, 'evidence':str(quarantine), 'recordSha256':actual['recordSha256']}
    pid, generation = int(sys.argv[3]), sys.argv[4]
    if op == 'acquire':
        deadline = time.monotonic() + int(sys.argv[5]) / 1000
        path.parent.mkdir(parents=True, exist_ok=True)
        while True:
            try:
                path.mkdir(mode=0o700)
                break
            except FileExistsError:
                if time.monotonic() >= deadline:
                    raise RuntimeError('lease_busy_or_incomplete')
                time.sleep(0.05)
        info = path.stat()
        owner_start = start(pid)
        if not owner_start:
            raise RuntimeError('owner_already_absent')
        value = {'schemaVersion':2,'pid':pid,'processStartTime':owner_start,'generation':generation,
                 'directoryDevice':info.st_dev,'directoryInode':info.st_ino,'children':[]}
        write(path, value)
        return value
    value, digest = snapshot(path)
    if value['pid'] != pid or value['generation'] != generation or start(pid) != value['processStartTime']:
        raise RuntimeError('lease_owner_changed')
    if op == 'reserve-child':
        if any(v['observed'] is not None for v in observe_children(value)):
            raise RuntimeError('lease_children_present')
        value['pendingChild'] = True
        write(path, value)
        return value
    if op == 'bind':
        child = json.loads(sys.argv[5])
        child_identity(child)
        if start(child['supervisorPid']) != child['supervisorStartTime']:
            raise RuntimeError('lease_child_supervisor_changed')
        value['children'].append(child)
        value['pendingChild'] = False
        write(path, value)
        return value
    if op == 'release':
        if any(v['observed'] is not None for v in observe_children(value)):
            raise RuntimeError('lease_children_present')
        current, current_digest = snapshot(path)
        if current != value or current_digest != digest:
            raise RuntimeError('lease_release_identity_changed')
        if sorted(p.name for p in path.iterdir()) != ['lease.json']:
            raise RuntimeError('lease_unknown_files')
        (path / 'lease.json').unlink()
        path.rmdir()
        return {'released':True,'generation':generation}
    raise RuntimeError('unknown_lease_operation')

def main_diagnose(path):
    previous = sys.argv
    try:
        sys.argv = [previous[0], 'diagnose', str(path)]
        return main()
    finally:
        sys.argv = previous
try:
    print(canonical(main()))
except Exception as error:
    print(canonical({'error':str(error)}), file=sys.stderr)
    sys.exit(75)
PY
fi
