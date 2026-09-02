#!/usr/bin/env python3
"""Session/FIFO and request-owned supervision. Only spawned groups are signalled.
Request mode uses stdin for lifetime/input frames and stdout for control/output frames.
"""
from __future__ import annotations
import argparse
import base64
import json
import os
import selectors
import signal
import subprocess
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path


def utc_now():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def atomic_json(path, payload):
    tmp = path.with_name(path.name + ".tmp-" + str(uuid.uuid4()))
    with tmp.open("x", encoding="utf-8") as file:
        file.write(json.dumps(payload, separators=(",", ":")) + "\n")
        file.flush()
        os.fsync(file.fileno())
    tmp.replace(path)


def append_lifecycle(path, payload):
    with path.open("a", encoding="utf-8") as file:
        file.write(json.dumps(payload, separators=(",", ":")) + "\n")


def process_start(pid):
    result = subprocess.run(["ps", "-p", str(pid), "-o", "lstart="], capture_output=True,
                            text=True, timeout=1, env={**os.environ, "LC_ALL": "C"})
    if result.returncode or not result.stdout.strip():
        raise RuntimeError("process_start_unavailable")
    return result.stdout.strip()


def group_members(group):
    try:
        result = subprocess.run(["ps", "-axo", "pid=,pgid=,stat="], capture_output=True, text=True, timeout=1)
        if result.returncode:
            return None
        return [int(fields[0]) for line in result.stdout.splitlines()
                if len(fields := line.split()) >= 3 and int(fields[1]) == group and not fields[2].startswith("Z")]
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return None


def reap_group(child):
    if child is None:
        return True, True, [], []
    failures = []
    members = group_members(child.pid)
    for sig, duration in ((signal.SIGTERM, 1.0), (signal.SIGKILL, 1.0)):
        if child.poll() is not None and members == []:
            break
        try:
            os.killpg(child.pid, sig)
        except ProcessLookupError:
            pass
        except OSError:
            failures.append("group_signal_failed")
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            child.poll()
            members = group_members(child.pid)
            if child.returncode is not None and members == []:
                break
            time.sleep(0.025)
    try:
        child.wait(timeout=0.25)
    except subprocess.TimeoutExpired:
        failures.append("process_reap_timeout")
    members = group_members(child.pid)
    survivors = ([{"kind": "process-group", "identity": str(child.pid), "observation": "unknown"}]
                 if members is None else [{"kind": "process", "identity": str(pid), "observation": "present"} for pid in members])
    return child.returncode is not None, members == [], survivors, failures


def identity_for(child, instance, generation):
    return {"pid": child.pid, "processStartTime": process_start(child.pid), "processInstanceId": instance,
            "processGroupId": child.pid, "supervisorPid": os.getpid(),
            "supervisorStartTime": process_start(os.getpid()), "sessionGeneration": generation}


def validate_native_lifecycle(value, identity, expected):
    if not isinstance(value, dict):
        raise ValueError("native_lifecycle_invalid")
    result = value.get("result", {}) if isinstance(value, dict) else {}
    native = result.get("native", {}) if isinstance(result, dict) else {}
    actual = result.get("identity", {}) if isinstance(result, dict) else {}
    if (not isinstance(result, dict) or not isinstance(native, dict) or not isinstance(actual, dict)
        or value.get("type") != "designResult" or type(value.get("protocolVersion")) is not int or value["protocolVersion"] != 2
        or "requestId" in value or "response" in value or result.get("operation") != "end"
        or result.get("lifecycle") is not True or type(result.get("schemaVersion")) is not int or result["schemaVersion"] != 1
        or result.get("shutdownReason") not in ("inputEof", "lifetimeExpired", "explicitEnd", "error")
        or result.get("launchNonce") != expected["launchNonce"] or result.get("policySha256") != expected["policySha256"]
        or any(actual.get(key) != identity[key] for key in ("pid", "processStartTime", "processInstanceId", "sessionGeneration"))
        or any(actual.get(key) != expected[key] for key in ("binarySha256", "manifestSha256"))
        or type(result.get("ok")) is not bool or type(result.get("ownedWindowsClosed")) is not bool
        or type(native.get("installed")) is not bool):
        raise ValueError("native_lifecycle_invalid")
    counts = [result.get("remainingWindows"), result.get("refusedEffects")]
    counts.extend(native.get(key) for key in ("openedWindows", "liveWindows", "automationWindows", "completedFrames", "readbackImages", "refusedOperations"))
    if (any(type(count) is not int or not 0 <= count <= 9007199254740991 for count in counts)
        or result["remainingWindows"] != native["liveWindows"] or native["liveWindows"] > native["openedWindows"]
        or result["ownedWindowsClosed"] != (result["ok"] and native["installed"] and native["liveWindows"] == 0 and native["automationWindows"] == 0)):
        raise ValueError("native_lifecycle_invalid")
    return value


def request_owned():
    child = None
    selector = selectors.DefaultSelector()
    failures = []
    identity = None
    cancelled = None
    pending = bytearray()
    pending_input = bytearray()
    control = bytearray()
    pending_limit = 1024 * 1024
    paused_sources = {}
    output_waiting = False
    input_waiting = False
    input_ended = False
    source_open = set()
    retained = 0
    maximum = 0
    configured = False
    owned_native = None
    native_lifecycle = None
    native_failure = None
    native_buffer = bytearray()
    owner_lost = False
    owner_pid = os.getppid()
    task_bound = False

    def task_bridge(operation, **payload):
        task = owned_native.get("task") if owned_native else None
        if not task:
            return
        root = Path(__file__).resolve().parents[2]
        try:
            # Metadata acquisition permits 5s (+2s process overhead), identity checks
            # permit 1s, and lease release permits 10s. Never kill a valid handoff at 3s.
            subprocess.run([task["helperExecutable"], str(root / "scripts/agentic/build-artifact.ts"), operation],
                input=json.dumps({**task, **payload}), text=True, capture_output=True, check=True, timeout=20,
                cwd=str(root), env=env)
        except subprocess.TimeoutExpired as error:
            raise TimeoutError(operation.replace("-", "_") + "_timeout") from error
        except subprocess.CalledProcessError as error:
            raise RuntimeError(operation.replace("-", "_") + "_failed") from error

    def observe_native(data):
        nonlocal native_lifecycle, native_failure
        if not owned_native or native_failure:
            return
        native_buffer.extend(data)
        while b"\n" in native_buffer:
            newline = native_buffer.index(b"\n")
            line = bytes(native_buffer[:newline])
            del native_buffer[:newline + 1]
            if len(line) > 6 * 1024 * 1024:
                native_failure = "native_lifecycle_line_limit"
                return
            try:
                value = json.loads(line)
                result = value.get("result") if isinstance(value, dict) else None
                if isinstance(result, dict) and ("lifecycle" in result or "shutdownReason" in result):
                    if native_lifecycle is not None or len(line) > 16 * 1024:
                        raise ValueError("native_lifecycle_duplicate_or_oversized")
                    native_lifecycle = validate_native_lifecycle(value, identity, owned_native)
            except (ValueError, TypeError, KeyError) as error:
                native_failure = str(error) or "native_lifecycle_invalid"
                return
        if len(native_buffer) > 6 * 1024 * 1024:
            native_failure = "native_lifecycle_line_limit"

    def detach_output():
        nonlocal owner_lost, output_waiting
        owner_lost = True
        pending.clear()
        if output_waiting:
            selector.unregister(1)
            output_waiting = False
        for channel, file in paused_sources.items():
            selector.register(file, selectors.EVENT_READ, channel)
        paused_sources.clear()

    def stop(_signum, _frame):
        nonlocal cancelled
        cancelled = "request_signal"

    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, stop)

    def encode(payload):
        return (json.dumps(payload, separators=(",", ":")) + "\n").encode()

    def emit(payload):
        data = encode(payload)
        deadline = time.monotonic() + 0.5
        while data:
            try:
                data = data[os.write(1, data):]
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise TimeoutError("control_write_timeout")
                time.sleep(0.005)

    def flush_output():
        nonlocal output_waiting
        if owner_lost:
            return
        if pending:
            try:
                count = os.write(1, pending)
                del pending[:count]
            except BlockingIOError:
                pass
            except BrokenPipeError:
                detach_output()
                return
        if pending and not output_waiting:
            selector.register(1, selectors.EVENT_WRITE, "output")
            output_waiting = True
        elif not pending and output_waiting:
            selector.unregister(1)
            output_waiting = False
        if len(pending) <= pending_limit // 2:
            for channel, file in paused_sources.items():
                selector.register(file, selectors.EVENT_READ, channel)
            paused_sources.clear()

    def capture_source(key, strict=True):
        nonlocal retained
        # Leave room for base64 expansion and the bounded JSON frame header.
        capacity = min(65536, (pending_limit - len(pending) - 128) * 3 // 4)
        if capacity < 1:
            selector.unregister(key.fileobj)
            paused_sources[key.data] = key.fileobj
            return
        try:
            data = os.read(key.fd, capacity)
        except OSError:
            if strict:
                raise
            data = b""
        if not data:
            selector.unregister(key.fileobj)
            source_open.discard(key.data)
            return
        if key.data == 1:
            observe_native(data)
        retained += len(data)
        if retained > maximum:
            if strict:
                raise RuntimeError("process_output_limit")
            return
        if owner_lost:
            return
        pending.extend(encode({"event": "output", "channel": "stdout" if key.data == 1 else "stderr",
                               "data": base64.b64encode(data).decode("ascii")}))

    def close_input():
        nonlocal input_waiting
        if child is not None and child.stdin is not None and not child.stdin.closed:
            if input_waiting:
                selector.unregister(child.stdin)
                input_waiting = False
            child.stdin.close()

    def flush_input():
        nonlocal input_waiting
        if pending_input:
            try:
                count = os.write(child.stdin.fileno(), pending_input)
                del pending_input[:count]
            except BlockingIOError:
                pass
        if pending_input and not input_waiting:
            selector.register(child.stdin, selectors.EVENT_WRITE, "stdin")
            input_waiting = True
        elif not pending_input and input_waiting:
            selector.unregister(child.stdin)
            input_waiting = False
        if input_ended and not pending_input:
            close_input()

    def read_control():
        nonlocal input_ended
        data = os.read(0, 65536)
        if not data:
            raise EOFError("request_owner_disappeared")
        control.extend(data)
        while b"\n" in control:
            newline = control.index(b"\n")
            if newline > 128 * 1024:
                raise ValueError("request_control_limit")
            command = json.loads(control[:newline])
            del control[:newline + 1]
            if command.get("event") == "close":
                raise EOFError("request_closed")
            if command.get("event") == "stdin-end" and not input_ended:
                input_ended = True
            elif command.get("event") == "stdin" and not input_ended:
                payload = command.get("data")
                if not isinstance(payload, str) or len(payload) > 87384:
                    raise ValueError("request_input_invalid")
                decoded = base64.b64decode(payload, validate=True)
                if len(decoded) > 65536 or len(pending_input) + len(decoded) > pending_limit:
                    raise ValueError("request_input_limit")
                pending_input.extend(decoded)
            else:
                raise ValueError("unexpected_request_control")
        if len(control) > 128 * 1024:
            raise ValueError("request_control_limit")

    try:
        os.set_blocking(1, False)
        os.set_blocking(0, False)
        selector.register(0, selectors.EVENT_READ, "lifetime")
        spec_bytes = bytearray()
        deadline = time.monotonic() + 5
        while b"\n" not in spec_bytes:
            if time.monotonic() >= deadline or cancelled:
                raise TimeoutError("request_spec_timeout")
            if not selector.select(0.05):
                continue
            data = os.read(0, 65536)
            if not data:
                raise EOFError("request_owner_disappeared")
            spec_bytes.extend(data)
            if len(spec_bytes) > pending_limit:
                raise ValueError("request_spec_limit")
        line, extra = bytes(spec_bytes).split(b"\n", 1)
        if extra:
            raise ValueError("unexpected_request_control")
        spec = json.loads(line)
        argv, cwd, env = spec["argv"], spec["cwd"], spec["env"]
        timeout, maximum = spec["timeoutMs"], spec["maxOutputBytes"]
        if (not isinstance(argv, list) or not argv or not all(isinstance(v, str) and "\0" not in v for v in argv)
            or not isinstance(cwd, str) or not isinstance(env, dict)
            or not all(isinstance(k, str) and isinstance(v, str) for k, v in env.items())
            or type(timeout) is not int or not 1 <= timeout <= 7200000
            or type(maximum) is not int or not 1 <= maximum <= 268435456):
            raise ValueError("invalid_owned_process_options")
        owned_native = spec.get("ownedNative")
        if owned_native is not None:
            if (not isinstance(owned_native, dict) or env.get("SCRIPT_KIT_OWNED_EVALUATION") != "1"
                or len(argv) < 2 or argv[-1] != "--owned-ui-evaluation"
                or any(owned_native.get(key) != env.get(name) for key, name in (
                    ("launchNonce", "SCRIPT_KIT_OWNED_EVALUATION_NONCE"), ("policySha256", "SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256"),
                    ("binarySha256", "SCRIPT_KIT_OWNED_EVALUATION_BINARY_SHA256"), ("manifestSha256", "SCRIPT_KIT_OWNED_EVALUATION_MANIFEST_SHA256")))):
                raise ValueError("owned_native_options_invalid")
        instance, generation = str(uuid.uuid4()), str(uuid.uuid4())
        child_env = {**env, "SCRIPT_KIT_PROCESS_INSTANCE_ID": instance,
                     "SCRIPT_KIT_AGENTIC_SESSION_GENERATION": generation,
                     "SCRIPT_KIT_SESSION_GENERATION": generation}
        gate_read, gate_write = os.pipe()
        try:
            child = subprocess.Popen([sys.executable, "-I", "-S", "-B", __file__, "--exec-child", str(gate_read), *argv],
                cwd=cwd, env=child_env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, start_new_session=True, pass_fds=(gate_read,))
            os.close(gate_read)
            gate_read = None
            identity = identity_for(child, instance, generation)
            if owned_native and owned_native.get("task"):
                task_bridge("native-task-bind", processIdentity=identity)
                task_bound = True
            emit({"event": "started", "identity": identity})
            os.write(gate_write, b"1")
        finally:
            if gate_read is not None:
                os.close(gate_read)
            os.close(gate_write)
        os.set_blocking(child.stdin.fileno(), False)
        for file, channel in ((child.stdout, 1), (child.stderr, 2)):
            os.set_blocking(file.fileno(), False)
            selector.register(file, selectors.EVENT_READ, channel)
            source_open.add(channel)
        configured = True
        deadline = time.monotonic() + timeout / 1000
        drain_deadline = None
        while True:
            if cancelled or time.monotonic() >= deadline:
                raise TimeoutError(cancelled or "process_timeout")
            for key, _ in selector.select(0.02):
                if key.data == "lifetime":
                    read_control()
                elif type(key.data) is int:
                    capture_source(key)
            flush_input()
            flush_output()
            if child.poll() is not None:
                drain_deadline = drain_deadline or time.monotonic() + 1
                if not source_open and not pending:
                    break
                if time.monotonic() >= drain_deadline:
                    raise TimeoutError("stream_drain_timeout")
    except BaseException as error:
        failures.append(str(error) or type(error).__name__)
        if str(error) == "request_owner_disappeared":
            detach_output()
    finally:
        close_input()
        if owned_native and configured:
            # The same supervisor keeps draining child stdout after owner death.
            # Only this native mode receives EOF grace; SDK/build termination is unchanged.
            grace = time.monotonic() + 3
            while time.monotonic() < grace and (child.poll() is None or source_open):
                for key, _ in selector.select(0.02):
                    if key.data == "lifetime":
                        selector.unregister(key.fileobj)
                    elif type(key.data) is int:
                        capture_source(key, strict=False)
                try:
                    flush_output()
                except OSError:
                    detach_output()
        exited, group_exited, survivors, reap_failures = reap_group(child)
        failures.extend(reap_failures)
        deadline = time.monotonic() + 0.5
        while configured and (source_open or pending) and time.monotonic() < deadline:
            for key, _ in selector.select(0.02):
                if key.data == "lifetime":
                    selector.unregister(key.fileobj)
                elif type(key.data) is int:
                    capture_source(key, strict=False)
            try:
                flush_output()
            except OSError:
                detach_output()
        drained = not source_open and not pending
        if child and not configured:
            drained = False
        if not drained:
            failures.append("streams_not_drained")
        if child:
            for file in (child.stdout, child.stderr):
                file.close()
        selector.close()
        if owned_native and native_buffer:
            native_failure = native_failure or "native_lifecycle_truncated_line"
        windows_closed = (native_lifecycle["result"]["ownedWindowsClosed"] if native_lifecycle and not native_failure else None)
        if owned_native and windows_closed is not True:
            failures.append(native_failure or "windows_not_observed_closed")
        native_references_finalized = (not owned_native or
            (windows_closed is True and child is not None and child.returncode == 0))
        if owned_native and not native_references_finalized:
            failures.append("native_references_not_finalized")
        cleanup = {"resourcesAcquired": child is not None, "processExited": exited,
            "processGroupExited": group_exited, "streamsDrained": drained, "logWriterClosed": True,
            "ownedWindowsClosed": windows_closed, "referencesFinalized": not task_bound and native_references_finalized,
            "closed": exited and group_exited and drained and not survivors and native_references_finalized,
            "survivors": survivors, "failureCodes": sorted(set(failures))}
        try:
            exit_code = child.returncode if child and child.returncode is not None else 70
            if failures and exit_code == 0:
                exit_code = 70
            if task_bound and (owner_lost or os.getppid() != owner_pid):
                try:
                    finalized = {**cleanup, "referencesFinalized": native_references_finalized}
                    task_bridge("native-task-finalize", processIdentity=identity, cleanup=finalized, exitCode=exit_code, nativeLifecycle=native_lifecycle)
                    cleanup = finalized
                except Exception:
                    cleanup.update(closed=False, referencesFinalized=False)
                    cleanup["failureCodes"].append("native_task_finalization_failed")
            if not pending:
                emit({"event": "done", "exitCode": exit_code,
                      "identity": identity, "cleanup": cleanup, "outputBytes": retained,
                      "nativeLifecycle": native_lifecycle, "nativeLifecycleFailure": native_failure})
        except (OSError, TimeoutError):
            pass
    return 0 if cleanup["closed"] else 70


def session_mode(argv):
    parser = argparse.ArgumentParser()
    for name in ("binary", "stdin-path", "stdout-path", "session-dir", "session-name", "generation"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args(argv)
    session_dir = Path(args.session_dir)
    child = stdin_file = stdout_file = None
    cancelled = False
    failure = None

    def stop(_signum, _frame):
        nonlocal cancelled
        cancelled = True

    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, stop)
    try:
        stdin_file = open(args.stdin_path, "rb", buffering=0)
        stdout_file = open(args.stdout_path, "ab", buffering=0)
        gate_read, gate_write = os.pipe()
        try:
            child = subprocess.Popen([sys.executable, "-B", __file__, "--exec-child", str(gate_read), args.binary],
                stdin=stdin_file, stdout=stdout_file, stderr=subprocess.STDOUT,
                env=os.environ.copy(), start_new_session=True, pass_fds=(gate_read,))
            os.close(gate_read)
            gate_read = None
            atomic_json(session_dir / "process-identity.json", identity_for(child,
                os.environ.get("SCRIPT_KIT_PROCESS_INSTANCE_ID", args.generation), args.generation))
            if (session_dir / "artifact-reference.json").exists():
                root = Path(__file__).resolve().parents[2]
                subprocess.run(["bun", str(root / "scripts/agentic/build-artifact.ts"), "session-pin", str(root), str(session_dir)], check=True, timeout=60)
            os.write(gate_write, b"1")
            (session_dir / "pid").write_text(f"{child.pid}\n", encoding="utf-8")
        finally:
            if gate_read is not None:
                os.close(gate_read)
            os.close(gate_write)
        while child.poll() is None and not cancelled:
            time.sleep(0.05)
    except BaseException as error:
        failure = str(error)
    finally:
        exited, group_exited, survivors, failures = reap_group(child)
        for file in (stdin_file, stdout_file):
            if file:
                file.close()
        code = child.returncode if child and child.returncode is not None else 70
        payload = {"schemaVersion": 1, "event": "app_process_exited", "session": args.session_name,
            "pid": child.pid if child else None, "exitStatus": f"signal:{-code}" if code < 0 else str(code),
            "exitCode": code if code >= 0 else None, "signal": -code if code < 0 else None,
            "cleanExit": code == 0 and exited and group_exited, "sessionGeneration": args.generation,
            "timestamp": utc_now(), "monotonicSeconds": round(time.monotonic(), 3),
            "cleanup": {"resourcesAcquired": child is not None, "processExited": exited,
                        "processGroupExited": group_exited, "streamsDrained": True, "logWriterClosed": True,
                        "ownedWindowsClosed": None, "referencesFinalized": True,
                        "closed": exited and group_exited, "survivors": survivors,
                        "failureCodes": failures + ([failure] if failure else [])}}
        atomic_json(session_dir / "app-exit.json", payload)
        if (session_dir / "managed-task.json").exists():
            root = Path(__file__).resolve().parents[2]
            try:
                subprocess.run(["bun", str(root / "scripts/agentic/build-artifact.ts"), "session-finalize", str(root), str(session_dir)], check=True, timeout=15)
            except Exception as error:
                payload["cleanup"].update(closed=False, referencesFinalized=False)
                payload["cleanup"]["failureCodes"].append(str(error))
                atomic_json(session_dir / "app-exit.json", payload)
        append_lifecycle(session_dir / "lifecycle.ndjson", payload)
    return 0 if exited and group_exited else 70


def stop_session(directory):
    path = Path(directory)
    if path.is_symlink() or (path / "process-identity.json").is_symlink():
        raise RuntimeError("session_identity_symlink")
    identity = json.loads((path / "process-identity.json").read_text())
    generation = (path / "generation").read_text().strip()
    if generation != identity["sessionGeneration"]:
        raise RuntimeError("session_generation_changed")
    supervisor = identity["supervisorPid"]
    try:
        actual = process_start(supervisor)
    except RuntimeError:
        actual = None
    if actual is not None:
        if actual != identity["supervisorStartTime"]:
            raise RuntimeError("session_supervisor_pid_reused")
        os.kill(supervisor, signal.SIGTERM)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        try:
            receipt = json.loads((path / "app-exit.json").read_text())
        except (OSError, ValueError):
            receipt = {}
        if (receipt.get("sessionGeneration") == generation and receipt.get("pid") == identity["pid"]
                and receipt.get("cleanup", {}).get("closed") is True and group_members(identity["processGroupId"]) == []):
            return 0
        time.sleep(0.05)
    raise RuntimeError("session_exact_cleanup_unproved")


def check_session(directory):
    path = Path(directory)
    identity_path = path / "process-identity.json"
    if path.is_symlink() or identity_path.is_symlink():
        return 1
    identity = json.loads(identity_path.read_text())
    if ((path / "generation").read_text().strip() != identity["sessionGeneration"]
            or (path / "pid").read_text().strip() != str(identity["pid"])):
        return 1
    return 0 if process_start(identity["pid"]) == identity["processStartTime"] else 1


def main():
    if sys.argv[1:2] == ["--check-session"]:
        return check_session(sys.argv[2])
    if sys.argv[1:2] == ["--stop-session"]:
        return stop_session(sys.argv[2])
    if sys.argv[1:2] == ["--exec-child"]:
        fd = int(sys.argv[2])
        if os.read(fd, 1) != b"1":
            return 70
        os.close(fd)
        os.environ["SCRIPT_KIT_PROCESS_START_TIME"] = process_start(os.getpid())
        os.execvpe(sys.argv[3], sys.argv[3:], os.environ)
    return request_owned() if sys.argv[1:] == ["--request-owned"] else session_mode(sys.argv[1:])


if __name__ == "__main__":
    sys.exit(main())
