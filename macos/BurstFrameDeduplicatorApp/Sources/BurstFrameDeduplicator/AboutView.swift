import AppKit
import BurstFrameAppCore
import SwiftUI

struct AboutView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    private let build = BuildDiagnostics.current
    private let system = SystemCapability.current

    var body: some View {
        VStack(spacing: 18) {
            Image(nsImage: NSApplication.shared.applicationIconImage)
                .resizable()
                .frame(width: 92, height: 92)
            VStack(spacing: 4) {
                Text(locale.text("appTitle"))
                    .font(.title2.weight(.semibold))
                Text(locale.text("versionValue", ["version": build.appVersion]))
                    .foregroundStyle(.secondary)
            }

            Grid(alignment: .leading, horizontalSpacing: 22, verticalSpacing: 7) {
                diagnosticRow("builtCommit", build.commit)
                diagnosticRow("rustToolchain", build.rustVersion)
                diagnosticRow("swiftToolchain", build.swiftVersion)
                diagnosticRow("commandLineTools", build.commandLineToolsVersion)
                Divider().gridCellColumns(2)
                diagnosticRow("macModel", system.modelName)
                diagnosticRow("operatingSystem", system.osVersion)
                diagnosticRow("processor", "\(system.cpuName) · \(system.logicalCPUCount)")
                diagnosticRow(
                    "memory",
                    ByteCountFormatter.string(fromByteCount: Int64(clamping: system.memoryBytes), countStyle: .memory)
                )
                diagnosticRow("graphics", system.gpuName)
                diagnosticRow("metalSupport", system.metalFamily)
            }
            .font(.callout)
            .textSelection(.enabled)
        }
        .padding(28)
        .frame(width: 560)
        .environment(\.locale, Locale(identifier: locale.appleLocaleIdentifier))
        .id(locale.code)
    }

    private func diagnosticRow(_ key: String, _ value: String) -> some View {
        GridRow {
            Text(locale.text(key))
                .foregroundStyle(.secondary)
            Text(value)
                .lineLimit(2)
                .truncationMode(.middle)
        }
    }
}
