import Foundation

struct RunCacheSummary: Equatable {
    var runCount = 0
    var bytes: UInt64 = 0
    var containsMovedRejects = false
}

@MainActor
final class RunCacheManager: ObservableObject {
    @Published private(set) var summary = RunCacheSummary()
    @Published private(set) var loading = false
    @Published var errorMessage: String?

    nonisolated static var runsDirectory: URL {
        let pictures = FileManager.default.urls(for: .picturesDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser
        return pictures.appendingPathComponent("Burst Frame Deduplicator Runs", isDirectory: true)
    }

    func refresh(excluding currentRunPath: String?) {
        loading = true
        let excluded = currentRunPath.map { URL(fileURLWithPath: $0).standardizedFileURL.path }
        DispatchQueue.global(qos: .utility).async {
            let result = Result { try Self.measure(excluding: excluded) }
            DispatchQueue.main.async { [weak self] in
                self?.loading = false
                switch result {
                case .success(let summary): self?.summary = summary
                case .failure(let error): self?.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func removePreviousRuns(excluding currentRunPath: String?) {
        loading = true
        let excluded = currentRunPath.map { URL(fileURLWithPath: $0).standardizedFileURL.path }
        DispatchQueue.global(qos: .utility).async {
            let result = Result {
                for directory in try Self.runDirectories(excluding: excluded) {
                    try FileManager.default.removeItem(at: directory)
                }
            }
            DispatchQueue.main.async { [weak self] in
                self?.loading = false
                switch result {
                case .success:
                    self?.summary = RunCacheSummary()
                case .failure(let error):
                    self?.errorMessage = error.localizedDescription
                }
            }
        }
    }

    nonisolated private static func measure(excluding currentRunPath: String?) throws -> RunCacheSummary {
        let directories = try runDirectories(excluding: currentRunPath)
        var summary = RunCacheSummary(runCount: directories.count)
        for directory in directories {
            summary.bytes += directorySize(directory)
            summary.containsMovedRejects = summary.containsMovedRejects || hasActiveMoveJournal(directory)
        }
        return summary
    }

    nonisolated private static func runDirectories(excluding currentRunPath: String?) throws -> [URL] {
        guard FileManager.default.fileExists(atPath: runsDirectory.path) else { return [] }
        let keys: Set<URLResourceKey> = [.isDirectoryKey, .isSymbolicLinkKey]
        return try FileManager.default.contentsOfDirectory(
            at: runsDirectory,
            includingPropertiesForKeys: Array(keys),
            options: [.skipsHiddenFiles]
        ).filter { url in
            let values = try? url.resourceValues(forKeys: keys)
            return values?.isDirectory == true
                && values?.isSymbolicLink != true
                && url.standardizedFileURL.path != currentRunPath
        }
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
            options: [.skipsHiddenFiles, .skipsPackageDescendants]
        ) else { return 0 }
        var total: UInt64 = 0
        for case let file as URL in enumerator {
            guard let values = try? file.resourceValues(forKeys: keys),
                  values.isRegularFile == true,
                  values.isSymbolicLink != true
            else { continue }
            total += UInt64(values.totalFileAllocatedSize ?? values.fileAllocatedSize ?? values.fileSize ?? 0)
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
