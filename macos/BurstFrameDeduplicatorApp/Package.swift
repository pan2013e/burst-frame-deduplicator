// swift-tools-version: 6.0

import Foundation
import PackageDescription

func commandOutput(_ executable: String, _ arguments: [String]) -> String? {
    let process = Process()
    let pipe = Pipe()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = arguments
    process.standardOutput = pipe
    guard (try? process.run()) != nil else { return nil }
    process.waitUntilExit()
    guard process.terminationStatus == 0 else { return nil }
    return String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines)
}

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let repositoryRoot = packageDirectory.deletingLastPathComponent().deletingLastPathComponent()
let releaseLibraryDirectory = repositoryRoot.appendingPathComponent("target/release").path
let commandLineTestingFrameworks = "/Library/Developer/CommandLineTools/Library/Developer/Frameworks"
let needsCommandLineTestingPath = commandOutput("/usr/bin/xcode-select", ["-p"]) == "/Library/Developer/CommandLineTools"
    && FileManager.default.fileExists(atPath: "\(commandLineTestingFrameworks)/Testing.framework")

let rustLinkerSettings: [LinkerSetting] = [
    .unsafeFlags([
        "-L\(releaseLibraryDirectory)",
        "-Xlinker", "-rpath", "-Xlinker", releaseLibraryDirectory,
        "-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks",
    ]),
]
let testSwiftSettings: [SwiftSetting] = needsCommandLineTestingPath
    ? [.unsafeFlags(["-F\(commandLineTestingFrameworks)"])]
    : []
let testLinkerSettings: [LinkerSetting] = rustLinkerSettings + (needsCommandLineTestingPath
    ? [.unsafeFlags([
        "-F\(commandLineTestingFrameworks)",
        "-Xlinker", "-rpath", "-Xlinker", commandLineTestingFrameworks,
    ])]
    : [])

let package = Package(
    name: "BurstFrameDeduplicatorApp",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "BurstFrameDeduplicator", targets: ["BurstFrameDeduplicator"]),
        .executable(name: "BurstFrameFFIBenchmark", targets: ["BurstFrameFFIBenchmark"]),
    ],
    targets: [
        .systemLibrary(
            name: "CBurstFrameDeduplicator",
            path: "Sources/CBurstFrameDeduplicator"
        ),
        .target(
            name: "BurstFrameAppCore",
            dependencies: ["CBurstFrameDeduplicator"],
            path: "Sources/BurstFrameAppCore"
        ),
        .executableTarget(
            name: "BurstFrameDeduplicator",
            dependencies: ["BurstFrameAppCore"],
            path: "Sources/BurstFrameDeduplicator",
            linkerSettings: rustLinkerSettings
        ),
        .executableTarget(
            name: "BurstFrameFFIBenchmark",
            dependencies: ["BurstFrameAppCore"],
            path: "Sources/BurstFrameFFIBenchmark",
            linkerSettings: rustLinkerSettings
        ),
        .testTarget(
            name: "BurstFrameAppCoreTests",
            dependencies: ["BurstFrameAppCore"],
            path: "Tests/BurstFrameAppCoreTests",
            swiftSettings: testSwiftSettings,
            linkerSettings: testLinkerSettings
        ),
    ],
    swiftLanguageModes: [.v5]
)
