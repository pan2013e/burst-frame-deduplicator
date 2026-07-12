import Combine
import Foundation

private struct LocaleFile: Decodable {
    let locale: String
    let languageName: String
    let macos: [String: String]
}

@MainActor
public final class LocaleCatalog: ObservableObject {
    public static let supportedCodes = ["en", "zh-CN"]

    @Published public var code: String {
        didSet {
            guard catalogs[code] != nil else {
                code = oldValue
                return
            }
            UserDefaults.standard.set(code, forKey: "locale")
        }
    }

    @Published public private(set) var loadError: String?
    private var catalogs: [String: LocaleFile] = [:]

    public init() {
        let preferred = UserDefaults.standard.string(forKey: "locale")
            ?? (Locale.preferredLanguages.first?.lowercased().hasPrefix("zh") == true ? "zh-CN" : "en")
        code = Self.supportedCodes.contains(preferred) ? preferred : "en"
        do {
            catalogs = try Self.loadCatalogs()
        } catch {
            loadError = error.localizedDescription
        }
    }

    public func text(_ key: String, _ values: [String: CustomStringConvertible] = [:]) -> String {
        let template = catalogs[code]?.macos[key] ?? catalogs["en"]?.macos[key] ?? key
        return values.reduce(template) { result, entry in
            result.replacingOccurrences(of: "{\(entry.key)}", with: entry.value.description)
        }
    }

    public func languageName(for code: String) -> String {
        catalogs[code]?.languageName ?? code
    }

    public var appleLocaleIdentifier: String {
        code == "zh-CN" ? "zh-Hans" : "en"
    }

    private static func loadCatalogs() throws -> [String: LocaleFile] {
        let directory = try localeDirectory()
        let decoder = JSONDecoder()
        return try Dictionary(uniqueKeysWithValues: supportedCodes.map { code in
            let data = try Data(contentsOf: directory.appendingPathComponent("\(code).json"))
            let catalog = try decoder.decode(LocaleFile.self, from: data)
            return (code, catalog)
        })
    }

    private static func localeDirectory() throws -> URL {
        let workingDirectory = URL(
            fileURLWithPath: FileManager.default.currentDirectoryPath,
            isDirectory: true
        )
        let repositoryFromPackageDirectory = workingDirectory
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let configured = ProcessInfo.processInfo.environment["BURST_DEDUP_LOCALES_DIR"].map {
            URL(fileURLWithPath: $0, isDirectory: true)
        }
        let candidates = [
            configured,
            Bundle.main.resourceURL?.appendingPathComponent("locales", isDirectory: true),
            workingDirectory.appendingPathComponent("locales", isDirectory: true),
            repositoryFromPackageDirectory.appendingPathComponent("locales", isDirectory: true),
        ].compactMap { $0 }
        if let directory = candidates.first(where: { candidate in
            supportedCodes.allSatisfy {
                FileManager.default.fileExists(atPath: candidate.appendingPathComponent("\($0).json").path)
            }
        }) {
            return directory
        }
        throw CocoaError(.fileNoSuchFile)
    }
}
