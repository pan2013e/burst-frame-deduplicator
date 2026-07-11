import Foundation

public enum TutorialOutcome: String, Codable, Equatable, Sendable {
    case completed
    case skipped
}

public struct TutorialProgressRecord: Codable, Equatable, Sendable {
    public let schemaVersion: Int
    public let outcome: TutorialOutcome
    public let finishedAt: String

    public init(schemaVersion: Int = 1, outcome: TutorialOutcome, finishedAt: String) {
        self.schemaVersion = schemaVersion
        self.outcome = outcome
        self.finishedAt = finishedAt
    }
}

public struct TutorialProgressStore {
    public static let recordKey = "tutorialProgressRecord"
    public static let legacyCompletionKey = "tutorialCompleted"

    private let readRecord: () -> String?
    private let writeRecord: (String) -> Void
    private let legacyCompleted: () -> Bool
    private let markLegacyComplete: () -> Void
    private let now: () -> String

    public init(
        defaults: UserDefaults = .standard,
        recordKey: String = Self.recordKey,
        legacyCompletionKey: String = Self.legacyCompletionKey
    ) {
        readRecord = { defaults.string(forKey: recordKey) }
        writeRecord = { defaults.set($0, forKey: recordKey) }
        legacyCompleted = { defaults.bool(forKey: legacyCompletionKey) }
        markLegacyComplete = { defaults.set(true, forKey: legacyCompletionKey) }
        now = { ISO8601DateFormatter().string(from: Date()) }
    }

    public init(
        readRecord: @escaping () -> String?,
        writeRecord: @escaping (String) -> Void,
        legacyCompleted: @escaping () -> Bool,
        markLegacyComplete: @escaping () -> Void,
        now: @escaping () -> String
    ) {
        self.readRecord = readRecord
        self.writeRecord = writeRecord
        self.legacyCompleted = legacyCompleted
        self.markLegacyComplete = markLegacyComplete
        self.now = now
    }

    public func currentRecord() -> TutorialProgressRecord? {
        if let encoded = readRecord(),
           let data = encoded.data(using: .utf8),
           let record = try? JSONDecoder().decode(TutorialProgressRecord.self, from: data),
           record.schemaVersion >= 1
        {
            return record
        }
        guard legacyCompleted() else { return nil }
        return record(.completed)
    }

    public func hasFinished() -> Bool {
        currentRecord() != nil
    }

    @discardableResult
    public func record(
        _ outcome: TutorialOutcome,
        finishedAt: String? = nil
    ) -> TutorialProgressRecord {
        let progress = TutorialProgressRecord(outcome: outcome, finishedAt: finishedAt ?? now())
        if let data = try? JSONEncoder().encode(progress),
           let encoded = String(data: data, encoding: .utf8)
        {
            writeRecord(encoded)
        }
        markLegacyComplete()
        return progress
    }
}
