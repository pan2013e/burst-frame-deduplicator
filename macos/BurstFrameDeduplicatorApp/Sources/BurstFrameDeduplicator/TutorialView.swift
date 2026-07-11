import BurstFrameAppCore
import SwiftUI

struct TutorialView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @State private var step = 0

    private let stepCount = 4

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(spacing: 11) {
                Image(systemName: "sparkles.rectangle.stack")
                    .font(.title2)
                    .foregroundStyle(.tint)
                    .symbolRenderingMode(.hierarchical)
                Text(locale.text("tutorialTitle"))
                    .font(.title2.weight(.semibold))
                Spacer()
                Text(locale.text("tutorialStep", ["current": step + 1, "total": stepCount]))
                    .font(.callout.monospacedDigit())
                    .foregroundStyle(.secondary)
            }

            tutorialDemo
                .frame(maxWidth: .infinity, minHeight: 205)

            VStack(alignment: .leading, spacing: 7) {
                Text(locale.text(titleKey))
                    .font(.title3.weight(.semibold))
                Text(locale.text(bodyKey))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Button(locale.text("tutorialSkip"), role: .cancel) {
                    model.dismissTutorial(outcome: .skipped)
                }
                Spacer()
                HStack(spacing: 6) {
                    ForEach(0 ..< stepCount, id: \.self) { index in
                        Capsule()
                            .fill(index == step ? Color.accentColor : Color.secondary.opacity(0.24))
                            .frame(width: index == step ? 18 : 6, height: 6)
                    }
                }
                Spacer()
                Button(locale.text("tutorialBack")) {
                    withAnimation(.snappy) { step -= 1 }
                }
                .disabled(step == 0)
                Button(locale.text(step == stepCount - 1 ? "tutorialDone" : "tutorialNext")) {
                    if step == stepCount - 1 {
                        model.dismissTutorial(outcome: .completed)
                    } else {
                        withAnimation(.snappy) { step += 1 }
                    }
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(26)
        .frame(width: 650, height: 500)
        .environment(\.locale, Locale(identifier: locale.appleLocaleIdentifier))
        .id(locale.code)
    }

    @ViewBuilder
    private var tutorialDemo: some View {
        switch step {
        case 0:
            VStack(spacing: 14) {
                Image(systemName: "folder.badge.plus")
                    .font(.system(size: 58, weight: .light))
                    .foregroundStyle(.tint)
                    .symbolRenderingMode(.hierarchical)
                Text(locale.text("tutorialDemoSource"))
                    .font(.headline)
                ProgressView(value: 0.68)
                    .frame(maxWidth: 330)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 8))
        case 1:
            HStack(spacing: 12) {
                demoFrame(symbol: "airplane", labelKey: "tutorialDemoReject", quality: 0.48, selected: false)
                demoFrame(symbol: "airplane", labelKey: "tutorialDemoKeep", quality: 0.91, selected: true)
                demoFrame(symbol: "airplane", labelKey: "tutorialDemoReview", quality: 0.72, selected: nil)
            }
        case 2:
            ZStack(alignment: .topTrailing) {
                RoundedRectangle(cornerRadius: 8)
                    .fill(Color(nsColor: .black).opacity(0.82))
                Image(systemName: "airplane")
                    .font(.system(size: 76, weight: .medium))
                    .foregroundStyle(.white.opacity(0.9))
                    .rotationEffect(.degrees(-8))
                HStack(spacing: 8) {
                    Image(systemName: "minus.magnifyingglass")
                    Image(systemName: "plus.magnifyingglass")
                    Image(systemName: "checkmark.square.fill")
                        .foregroundStyle(.green)
                }
                .padding(12)
                .foregroundStyle(.white)
            }
        default:
            HStack(spacing: 22) {
                Label(locale.text("tutorialDemoReject"), systemImage: "photo.stack")
                Image(systemName: "arrow.right")
                    .foregroundStyle(.secondary)
                Label(locale.text("move"), systemImage: "tray.and.arrow.down.fill")
                    .foregroundStyle(.red)
                Image(systemName: "arrow.uturn.backward")
                    .foregroundStyle(.blue)
            }
            .font(.title3.weight(.medium))
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 8))
        }
    }

    private func demoFrame(symbol: String, labelKey: String, quality: Double, selected: Bool?) -> some View {
        VStack(alignment: .leading, spacing: 9) {
            ZStack {
                RoundedRectangle(cornerRadius: 6)
                    .fill(Color.secondary.opacity(0.12))
                Image(systemName: symbol)
                    .font(.system(size: 38, weight: .medium))
                    .rotationEffect(.degrees(quality > 0.8 ? -8 : 5))
            }
            HStack(spacing: 6) {
                Image(systemName: selected == true ? "checkmark.square.fill" : selected == false ? "square" : "minus.square.fill")
                    .foregroundStyle(selected == true ? .green : selected == nil ? .orange : .secondary)
                Text(locale.text(labelKey))
                    .font(.callout.weight(.medium))
            }
            ContinuousLevelBar(value: quality)
        }
        .padding(10)
        .background(.background, in: RoundedRectangle(cornerRadius: 7))
        .overlay(
            RoundedRectangle(cornerRadius: 7)
                .stroke(Color(nsColor: .separatorColor).opacity(0.55))
        )
    }

    private var titleKey: String {
        ["tutorialScanTitle", "tutorialSuggestionsTitle", "tutorialInspectTitle", "tutorialMoveTitle"][step]
    }

    private var bodyKey: String {
        ["tutorialScanBody", "tutorialSuggestionsBody", "tutorialInspectBody", "tutorialMoveBody"][step]
    }
}
