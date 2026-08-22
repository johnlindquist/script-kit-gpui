import Foundation
import CoreServices
import Dispatch
import Darwin

private let bytesPerGiB: UInt64 = 1024 * 1024 * 1024
private let isoFormatter = ISO8601DateFormatter()

private func die(_ message: String) -> Never {
    fputs("[disk-space-cargo-watcher] fatal: \(message)\n", stderr)
    exit(2)
}

private func normalizePath(_ path: String) -> String {
    let expanded = NSString(string: path).expandingTildeInPath
    return URL(fileURLWithPath: expanded).resolvingSymlinksInPath().standardizedFileURL.path
}

private func defaultStateDir() -> String {
    let home = FileManager.default.homeDirectoryForCurrentUser.path
    return "\(home)/Library/Application Support/script-kit-gpui/disk-space-cargo-watcher"
}

private func humanGiB(_ bytes: UInt64) -> String {
    return String(format: "%.1fGiB", Double(bytes) / Double(bytesPerGiB))
}

private func pidIsAlive(_ pid: Int32) -> Bool {
    if pid <= 0 { return false }
    if kill(pid, 0) == 0 { return true }
    return errno == EPERM
}

private struct Config {
    var repoRoot: String = "/Users/johnlindquist/dev/script-kit-gpui"
    var thresholdGiB: UInt64 = 25
    var targetFreeGiB: UInt64 = 35
    var cooldownSeconds: TimeInterval = 1800
    var debounceSeconds: TimeInterval = 15
    var fseventLatencySeconds: CFTimeInterval = 5
    var cleanupScript: String = ""
    var stateDir: String = defaultStateDir()
}

private func parseConfig() -> Config {
    var config = Config()
    let args = CommandLine.arguments
    var i = 1
    func takeValue(after flag: String) -> String {
        i += 1
        guard i < args.count else { die("missing value after \(flag)") }
        return args[i]
    }
    while i < args.count {
        let arg = args[i]
        switch arg {
        case "--repo":
            config.repoRoot = takeValue(after: arg)
        case "--threshold-gib":
            guard let value = UInt64(takeValue(after: arg)) else { die("invalid --threshold-gib") }
            config.thresholdGiB = value
        case "--target-free-gib":
            guard let value = UInt64(takeValue(after: arg)) else { die("invalid --target-free-gib") }
            config.targetFreeGiB = value
        case "--cooldown-seconds":
            guard let value = Double(takeValue(after: arg)) else { die("invalid --cooldown-seconds") }
            config.cooldownSeconds = value
        case "--debounce-seconds":
            guard let value = Double(takeValue(after: arg)) else { die("invalid --debounce-seconds") }
            config.debounceSeconds = value
        case "--fsevent-latency-seconds":
            guard let value = Double(takeValue(after: arg)) else { die("invalid --fsevent-latency-seconds") }
            config.fseventLatencySeconds = value
        case "--cleanup":
            config.cleanupScript = takeValue(after: arg)
        case "--state-dir":
            config.stateDir = takeValue(after: arg)
        case "--help", "-h":
            print("""
            Usage: disk-space-cargo-watcher --repo PATH --threshold-gib 25 --target-free-gib 35 \\
                   --cooldown-seconds 1800 --debounce-seconds 15 --cleanup PATH --state-dir PATH
            """)
            exit(0)
        default:
            die("unknown argument: \(arg)")
        }
        i += 1
    }
    config.repoRoot = normalizePath(config.repoRoot)
    config.stateDir = normalizePath(config.stateDir)
    if config.cleanupScript.isEmpty {
        config.cleanupScript = "\(config.repoRoot)/scripts/agentic/disk-space-cargo-emergency-clean.sh"
    }
    config.cleanupScript = normalizePath(config.cleanupScript)
    return config
}

fileprivate final class DiskSpaceCargoWatcher {
    private let config: Config
    private let fileManager = FileManager.default
    private let queue = DispatchQueue(label: "script-kit.disk-space-cargo-watcher")
    private var stream: FSEventStreamRef?
    private var debounceWorkItem: DispatchWorkItem?
    private var cleanupRunning = false
    private var cleanupProcess: Process?

    private var thresholdBytes: UInt64 { config.thresholdGiB * bytesPerGiB }
    private var watchedPaths: [String] {
        [
            "\(config.repoRoot)/target",
            "\(config.repoRoot)/target-agent"
        ]
    }
    private var lastTriggerPath: String { "\(config.stateDir)/last-trigger-epoch" }
    private var cleanupLockDir: String { "\(config.stateDir)/cleanup.lock" }

    init(config: Config) {
        self.config = config
    }

    func log(_ message: String) {
        print("[disk-space-cargo-watcher] \(isoFormatter.string(from: Date())) \(message)")
        fflush(stdout)
    }

    func start() {
        ensureDirectories()
        log("starting repo=\(config.repoRoot) threshold=\(config.thresholdGiB)GiB targetFree=\(config.targetFreeGiB)GiB cooldown=\(Int(config.cooldownSeconds))s debounce=\(Int(config.debounceSeconds))s cleanup=\(config.cleanupScript)")

        queue.async { [weak self] in
            self?.checkAndMaybeLaunchCleanup(reason: "startup")
        }

        var context = FSEventStreamContext(
            version: 0,
            info: UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque()),
            retain: nil,
            release: nil,
            copyDescription: nil
        )

        let flags = FSEventStreamCreateFlags(
            kFSEventStreamCreateFlagUseCFTypes |
            kFSEventStreamCreateFlagNoDefer |
            kFSEventStreamCreateFlagWatchRoot
        )

        guard let createdStream = FSEventStreamCreate(
            kCFAllocatorDefault,
            fseventsCallback,
            &context,
            watchedPaths as CFArray,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            config.fseventLatencySeconds,
            flags
        ) else {
            die("FSEventStreamCreate failed")
        }

        let fsQueue = DispatchQueue(label: "script-kit.disk-space-cargo-watcher.fsevents")
        FSEventStreamSetDispatchQueue(createdStream, fsQueue)

        guard FSEventStreamStart(createdStream) else {
            die("FSEventStreamStart failed")
        }

        stream = createdStream
        log("watching \(watchedPaths.joined(separator: ", "))")

        dispatchMain()
    }

    func stopAndExit(_ status: Int32) -> Never {
        log("stopping status=\(status)")
        if let stream {
            FSEventStreamStop(stream)
            FSEventStreamInvalidate(stream)
            FSEventStreamRelease(stream)
            self.stream = nil
        }
        exit(status)
    }

    func handleFSEvents(paths: [String]) {
        queue.async { [weak self] in
            guard let self else { return }
            let sample = paths.prefix(5).joined(separator: ", ")
            self.log("fsevents count=\(paths.count) sample=\(sample)")

            self.debounceWorkItem?.cancel()
            let item = DispatchWorkItem { [weak self] in
                self?.checkAndMaybeLaunchCleanup(reason: "fsevents")
            }
            self.debounceWorkItem = item
            let delay = DispatchTimeInterval.milliseconds(max(0, Int(self.config.debounceSeconds * 1000)))
            self.queue.asyncAfter(deadline: .now() + delay, execute: item)
        }
    }

    private func ensureDirectories() {
        do {
            try fileManager.createDirectory(atPath: config.stateDir, withIntermediateDirectories: true)
            for path in watchedPaths {
                try fileManager.createDirectory(atPath: path, withIntermediateDirectories: true)
            }
        } catch {
            die("failed creating watcher directories: \(error)")
        }
    }

    private func freeBytes() throws -> UInt64 {
        let attrs = try fileManager.attributesOfFileSystem(forPath: config.repoRoot)
        guard let free = attrs[FileAttributeKey.systemFreeSize] as? NSNumber else {
            throw NSError(domain: "DiskSpaceCargoWatcher", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "systemFreeSize unavailable"
            ])
        }
        return free.uint64Value
    }

    private func readLastTriggerEpoch() -> TimeInterval? {
        guard let raw = try? String(contentsOfFile: lastTriggerPath, encoding: .utf8) else { return nil }
        return TimeInterval(raw.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    private func writeLastTriggerEpoch(_ epoch: TimeInterval) {
        do {
            try "\(epoch)\n".write(toFile: lastTriggerPath, atomically: true, encoding: .utf8)
        } catch {
            log("warning failed writing last trigger: \(error)")
        }
    }

    private func cleanupLockIsActive() -> Bool {
        var isDir: ObjCBool = false
        guard fileManager.fileExists(atPath: cleanupLockDir, isDirectory: &isDir),
              isDir.boolValue else { return false }
        let pidPath = "\(cleanupLockDir)/pid"
        if let raw = try? String(contentsOfFile: pidPath, encoding: .utf8),
           let pid = Int32(raw.trimmingCharacters(in: .whitespacesAndNewlines)),
           pidIsAlive(pid) {
            return true
        }
        log("removing stale cleanup lock \(cleanupLockDir)")
        try? fileManager.removeItem(atPath: cleanupLockDir)
        return false
    }

    private func checkAndMaybeLaunchCleanup(reason: String) {
        let free: UInt64
        do {
            free = try freeBytes()
        } catch {
            log("disk check failed reason=\(reason) error=\(error)")
            return
        }

        if free >= thresholdBytes {
            log("free ok reason=\(reason) free=\(humanGiB(free)) threshold=\(config.thresholdGiB)GiB")
            return
        }

        log("free below threshold reason=\(reason) free=\(humanGiB(free)) threshold=\(config.thresholdGiB)GiB")

        if cleanupRunning {
            log("cleanup already running in watcher process")
            return
        }

        if cleanupLockIsActive() {
            log("cleanup lock active; not launching another Claude session")
            return
        }

        let now = Date().timeIntervalSince1970
        if let last = readLastTriggerEpoch(), config.cooldownSeconds > 0 {
            let elapsed = now - last
            if elapsed < config.cooldownSeconds {
                log("cooldown active elapsed=\(Int(elapsed))s cooldown=\(Int(config.cooldownSeconds))s")
                return
            }
        }

        launchCleanup(reason: reason, freeBytes: free)
    }

    private func makeCleanupEnv() -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        env["HOME"] = home
        env["PATH"] = "/Users/johnlindquist/.local/bin:\(home)/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        env["SCRIPT_KIT_REPO_ROOT"] = config.repoRoot
        env["SCRIPT_KIT_WATCHER_STATE_DIR"] = config.stateDir
        env["SCRIPT_KIT_FREE_THRESHOLD_GIB"] = "\(config.thresholdGiB)"
        env["SCRIPT_KIT_TARGET_FREE_GIB"] = "\(config.targetFreeGiB)"
        env["SCRIPT_KIT_ALLOW_SHARED_INCREMENTAL_EVICTION"] = "0"
        env["SCRIPT_KIT_ALLOW_CLEANUP_AGENT"] = "0"
        return env
    }

    private func launchFallbackCleanup(reason: String) {
        let emergencyScript = "\(config.repoRoot)/scripts/agentic/disk-space-cargo-emergency-clean.sh"
        guard fileManager.isExecutableFile(atPath: emergencyScript) else {
            log("fallback: emergency script not found at \(emergencyScript)")
            return
        }
        log("fallback: running emergency clean directly (Claude unavailable)")
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.currentDirectoryURL = URL(fileURLWithPath: config.repoRoot)
        process.arguments = [
            emergencyScript, "--apply",
            "--repo", config.repoRoot,
            "--threshold-gib", "\(config.thresholdGiB)",
            "--target-free-gib", "\(config.targetFreeGiB)",
            "--state-dir", config.stateDir,
            "--reason", "fallback-\(reason)"
        ]
        process.environment = makeCleanupEnv()

        cleanupRunning = true
        process.terminationHandler = { [weak self] terminated in
            self?.queue.async {
                self?.cleanupRunning = false
                let status = terminated.terminationStatus
                self?.log("fallback cleanup exited pid=\(terminated.processIdentifier) status=\(status)")
                if status == 0 {
                    self?.writeLastTriggerEpoch(Date().timeIntervalSince1970)
                }
            }
        }
        do {
            try process.run()
            log("fallback cleanup launched pid=\(process.processIdentifier)")
        } catch {
            cleanupRunning = false
            log("fallback cleanup launch failed: \(error)")
        }
    }

    private func launchCleanup(reason: String, freeBytes: UInt64) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/bash")
        process.currentDirectoryURL = URL(fileURLWithPath: config.repoRoot)
        process.arguments = [
            config.cleanupScript,
            "--apply",
            "--repo", config.repoRoot,
            "--threshold-gib", "\(config.thresholdGiB)",
            "--target-free-gib", "\(config.targetFreeGiB)",
            "--state-dir", config.stateDir,
            "--reason", reason
        ]
        process.environment = makeCleanupEnv()

        cleanupRunning = true
        cleanupProcess = process

        process.terminationHandler = { [weak self] terminated in
            self?.queue.async {
                let status = terminated.terminationStatus
                self?.cleanupRunning = false
                self?.cleanupProcess = nil
                self?.log("cleanup process exited pid=\(terminated.processIdentifier) status=\(status)")
                self?.writeLastTriggerEpoch(Date().timeIntervalSince1970)
                if status == 0 {
                    self?.log("deterministic lock-safe cleanup completed")
                } else {
                    self?.log("lock-safe cleanup could not reach its disk target; active builds and warm caches remain protected")
                }
            }
        }

        do {
            try process.run()
            log("launched cleanup pid=\(process.processIdentifier) reason=\(reason) free=\(humanGiB(freeBytes))")
        } catch {
            cleanupRunning = false
            cleanupProcess = nil
            log("failed launching cleanup: \(error)")
        }
    }
}

private func fseventsCallback(
    streamRef: ConstFSEventStreamRef,
    clientCallBackInfo: UnsafeMutableRawPointer?,
    numEvents: Int,
    eventPaths: UnsafeMutableRawPointer,
    eventFlags: UnsafePointer<FSEventStreamEventFlags>,
    eventIds: UnsafePointer<FSEventStreamEventId>
) {
    guard let info = clientCallBackInfo else { return }
    let watcher = Unmanaged<DiskSpaceCargoWatcher>
        .fromOpaque(info)
        .takeUnretainedValue()
    let paths = unsafeBitCast(eventPaths, to: NSArray.self) as? [String] ?? []
    watcher.handleFSEvents(paths: paths)
}

private let watcher = DiskSpaceCargoWatcher(config: parseConfig())

var retainedSignalSources: [DispatchSourceSignal] = []
private func installSignal(_ signalNumber: Int32) {
    signal(signalNumber, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: signalNumber, queue: .main)
    source.setEventHandler {
        watcher.stopAndExit(0)
    }
    source.resume()
    retainedSignalSources.append(source)
}

installSignal(SIGTERM)
installSignal(SIGINT)
watcher.start()
