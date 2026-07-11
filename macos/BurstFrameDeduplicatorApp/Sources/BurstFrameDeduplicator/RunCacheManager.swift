import Foundation

private let registeredRunsKey = "registeredRunPaths"

struct RunCacheSummary: Equatable {
    var runCount = 0
    var bytes: UInt64 = 0
    var containsMovedRejects = false
}

struct RunLibraryEntry: Identifiable, Equatable, Sendable {
    let path: String
    let name: String
    let sourcePath: String
    let createdAt: Date?
    let assets: Int
    let bytes: UInt64
    let containsMovedRejects: Bool
    let sourceAvailable: Bool

    var id: String { path }
}

@MainActor
final class RunCacheManager: ObservableObject {
    @Published private(set) var entries: [RunLibraryEntry] = []
    @Published private(set) var summary = RunCacheSummary()
    @Published private(set) var loading = false
    @Published var selectedPaths: Set<String> = []
    @Published var errorMessage: String?

    private var resultRoots: [String] = []
    private var excludedPath: String?

    nonisolated static var defaultRunsDirectory: URL {
        let pictures = FileManager.default.urls(for: .picturesDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser
        return pictures.appendingPathComponent("Burst Frame Deduplicator Runs", isDirectory: true)
    }

    nonisolated static func registerRun(_ path: String) {
        let normalized = URL(fileURLWithPath: path).standardizedFileURL.path
        var paths = Set(UserDefaults.standard.stringArray(forKey: registeredRunsKey) ?? [])
        paths.insert(normalized)
        UserDefaults.standard.set(paths.sorted(), forKey: registeredRunsKey)
    }

    nonisolated static func replaceRegisteredRun(previous: String, relocated: String) {
        let previous = URL(fileURLWithPath: previous).standardizedFileURL.path
        let relocated = URL(fileURLWithPath: relocated).standardizedFileURL.path
        var paths = Set(UserDefaults.standard.stringArray(forKey: registeredRunsKey) ?? [])
        paths.remove(previous)
        paths.insert(relocated)
        UserDefaults.standard.set(paths.sorted(), forKey: registeredRunsKey)
    }

    func refresh(resultRoots: [String], excluding currentRunPath: String? = nil) {
        self.resultRoots = resultRoots
        excludedPath = currentRunPath.map { URL(fileURLWithPath: $0).standardizedFileURL.path }
        loading = true
        let excludedPath = excludedPath
        DispatchQueue.global(qos: .utility).async {
            let result = Result { try Self.loadEntries(resultRoots: resultRoots) }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                loading = false
                switch result {
                case .success(let loaded):
                    entries = loaded
                    updateSummary(excluding: excludedPath)
                    selectedPaths = Set(loaded.lazy.map(\.path).filter { $0 != excludedPath })
                case .failure(let error):
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    func removeSelected() {
        let targets = entries.filter { selectedPaths.contains($0.path) && $0.path != excludedPath }
        guard !targets.isEmpty else { return }
        loading = true
        DispatchQueue.global(qos: .utility).async {
            let result = Result {
                for entry in targets {
                    let directory = URL(fileURLWithPath: entry.path, isDirectory: true)
                    guard FileManager.default.fileExists(
                        atPath: directory.appendingPathComponent("manifest.json").path
                    ) else {
                        throw CocoaError(.fileReadNoSuchFile)
                    }
                    let values = try directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
                    guard values.isDirectory == true, values.isSymbolicLink != true else {
                        throw CocoaError(.fileReadUnsupportedScheme)
                    }
                    try FileManager.default.removeItem(at: directory)
                    Self.unregisterRun(entry.path)
                }
            }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                loading = false
                switch result {
                case .success:
                    refresh(resultRoots: resultRoots, excluding: excludedPath)
                case .failure(let error):
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    func setAllSelected(_ selected: Bool) {
        selectedPaths = selected
            ? Set(entries.lazy.map(\.path).filter { $0 != self.excludedPath })
            : []
    }

    func toggleSelection(_ path: String) {
        guard path != excludedPath else { return }
        if selectedPaths.contains(path) {
            selectedPaths.remove(path)
        } else {
            selectedPaths.insert(path)
        }
    }

    var selectedSummary: RunCacheSummary {
        summarize(entries.filter { selectedPaths.contains($0.path) && $0.path != excludedPath })
    }

    nonisolated private static func unregisterRun(_ path: String) {
        let normalized = URL(fileURLWithPath: path).standardizedFileURL.path
        var paths = Set(UserDefaults.standard.stringArray(forKey: registeredRunsKey) ?? [])
        paths.remove(normalized)
        UserDefaults.standard.set(paths.sorted(), forKey: registeredRunsKey)
    }

    private func updateSummary(excluding: String?) {
        summary = summarize(entries.filter { $0.path != excluding })
    }

    private func summarize(_ entries: [RunLibraryEntry]) -> RunCacheSummary {
        RunCacheSummary(
            runCount: entries.count,
            bytes: entries.reduce(0) { $0.saturatingAdding($1.bytes) },
            containsMovedRejects: entries.contains(where: \.containsMovedRejects)
        )
    }

    nonisolated private static func loadEntries(resultRoots: [String]) throws -> [RunLibraryEntry] {
        var candidatePaths = Set(UserDefaults.standard.stringArray(forKey: registeredRunsKey) ?? [])
        let roots = Set(resultRoots + [defaultRunsDirectory.path])
        for rootPath in roots {
            let root = URL(fileURLWithPath: rootPath, isDirectory: true)
            guard FileManager.default.fileExists(atPath: root.path) else { continue }
            let children = try FileManager.default.contentsOfDirectory(
                at: root,
                includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey],
                options: [.skipsHiddenFiles]
            )
            for child in children {
                let values = try? child.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
                if values?.isDirectory == true, values?.isSymbolicLink != true {
                    candidatePaths.insert(child.standardizedFileURL.path)
                }
            }
        }

        var entries: [RunLibraryEntry] = []
        var validPaths = Set<String>()
        for path in candidatePaths {
            guard let entry = readEntry(at: URL(fileURLWithPath: path, isDirectory: true)) else { continue }
            entries.append(entry)
            validPaths.insert(entry.path)
        }
        UserDefaults.standard.set(validPaths.sorted(), forKey: registeredRunsKey)
        return entries.sorted {
            switch ($0.createdAt, $1.createdAt) {
            case let (left?, right?) where left != right: return left > right
            default: return $0.name.localizedStandardCompare($1.name) == .orderedDescending
            }
        }
    }

    nonisolated private static func readEntry(at directory: URL) -> RunLibraryEntry? {
        let manifestURL = directory.appendingPathComponent("manifest.json")
        guard let values = try? directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey]),
              values.isDirectory == true,
              values.isSymbolicLink != true,
              let data = try? Data(contentsOf: manifestURL),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let sourcePath = object["root"] as? String
        else { return nil }

        let summary = object["summary"] as? [String: Any]
        let assets = summary?["discovered_assets"] as? Int ?? 0
        let createdAt = (object["created_at"] as? String).flatMap(parseDate)
        let normalized = directory.standardizedFileURL.path
        return RunLibraryEntry(
            path: normalized,
            name: directory.lastPathComponent,
            sourcePath: sourcePath,
            createdAt: createdAt,
            assets: assets,
            bytes: directorySize(directory),
            containsMovedRejects: hasActiveMoveJournal(directory),
            sourceAvailable: FileManager.default.fileExists(atPath: sourcePath)
        )
    }

    nonisolated private static func parseDate(_ value: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.date(from: value) ?? ISO8601DateFormatter().date(from: value)
    }

    nonisolated private static func directorySize(_ directory: URL) -> UInt64 {
        let keys: Set<URLResourceKey> = [
            .isRegularFileKey,
            .isSymbolicLinkKey,
            .totalFileAllocatedSizeKey,
            .fileAllocatedSizeKey,
            .fileSizeKey,
        ]
        guard let enumerator = FileManager.default.enumerator(
            at: directory,
            includingPropertiesForKeys: Array(keys),
            options: [.skipsPackageDescendants]
        ) else { return 0 }
        var total: UInt64 = 0
        for case let file as URL in enumerator {
            guard let values = try? file.resourceValues(forKeys: keys),
                  values.isRegularFile == true,
                  values.isSymbolicLink != true
            else { continue }
            total = total.saturatingAdding(
                UInt64(values.totalFileAllocatedSize ?? values.fileAllocatedSize ?? values.fileSize ?? 0)
            )
        }
        return total
    }

    nonisolated private static func hasActiveMoveJournal(_ directory: URL) -> Bool {
        let stateURL = directory.appendingPathComponent("move_state.json")
        guard let data = try? Data(contentsOf: stateURL),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let records = object["records"] as? [[String: Any]]
        else { return false }
        return records.contains { $0["restored_at"] is NSNull || $0["restored_at"] == nil }
    }
}

private extension UInt64 {
    func saturatingAdding(_ other: UInt64) -> UInt64 {
        let (result, overflow) = addingReportingOverflow(other)
        return overflow ? .max : result
    }
}
