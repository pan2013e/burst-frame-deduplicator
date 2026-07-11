import AppKit
import BurstFrameAppCore
import SwiftUI

struct NativeImageViewer: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var model: AppModel
    @State private var image: NSImage?
    @State private var loading = true
    @State private var failed = false
    @State private var scale = 1.0
    @State private var gestureScale = 1.0
    @State private var offset = CGSize.zero
    @State private var dragOrigin = CGSize.zero
    @FocusState private var focused: Bool
    private let bridge = RustBridge()

    var body: some View {
        VStack(spacing: 0) {
            viewerToolbar
            Divider()
            GeometryReader { geometry in
                ZStack {
                    Color(nsColor: .underPageBackgroundColor)
                    if loading {
                        ProgressView(locale.text("loadingPreview"))
                            .controlSize(.large)
                    } else if failed || image == nil {
                        ContentUnavailableView(
                            locale.text("previewUnavailable"),
                            systemImage: "exclamationmark.triangle"
                        )
                    } else if let image {
                        Image(nsImage: image)
                            .resizable()
                            .scaledToFit()
                            .frame(width: geometry.size.width, height: geometry.size.height)
                            .scaleEffect(scale * gestureScale)
                            .offset(x: offset.width, y: offset.height)
                            .gesture(magnificationGesture.simultaneously(with: dragGesture))
                    }
                }
                .clipped()
            }
        }
        .frame(minWidth: 820, minHeight: 620)
        .focusable()
        .focused($focused)
        .onAppear { focused = true }
        .onKeyPress(.leftArrow) {
            navigate(-1)
            return .handled
        }
        .onKeyPress(.rightArrow) {
            navigate(1)
            return .handled
        }
        .onKeyPress(.escape) {
            close()
            return .handled
        }
        .task(id: model.viewerAssetID) {
            await loadCurrentImage()
        }
    }

    private var viewerToolbar: some View {
        HStack(spacing: 10) {
            Text(currentAsset?.representative.relPath ?? "")
                .font(.headline)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            if let asset = currentAsset {
                HStack(spacing: 5) {
                    TriStateCheckbox(
                        state: model.finalAction(for: asset),
                        accessibilityLabel: locale.text("keep")
                    ) { model.setDecision($0, for: asset) }
                    .fixedSize()
                    Text(locale.text("keep"))
                }
            }
            Divider().frame(height: 22)
            iconButton("chevron.left", help: locale.text("previousFrame"), disabled: adjacent(-1) == nil) {
                navigate(-1)
            }
            iconButton("chevron.right", help: locale.text("nextFrame"), disabled: adjacent(1) == nil) {
                navigate(1)
            }
            iconButton("minus.magnifyingglass", help: locale.text("zoomOut")) { zoom(0.8) }
            iconButton("plus.magnifyingglass", help: locale.text("zoomIn")) { zoom(1.25) }
            iconButton("arrow.up.left.and.arrow.down.right", help: locale.text("fit")) { resetTransform() }
            iconButton("xmark", help: locale.text("close"), action: close)
        }
        .padding(.horizontal, 12)
        .frame(height: 48)
        .background(.bar)
    }

    private var currentAsset: AssetRecord? {
        guard let id = model.viewerAssetID else { return nil }
        return model.assetsByID[id]
    }

    private var magnificationGesture: some Gesture {
        MagnifyGesture()
            .onChanged { value in gestureScale = value.magnification }
            .onEnded { value in
                scale = max(0.08, min(10, scale * value.magnification))
                gestureScale = 1
            }
    }

    private var dragGesture: some Gesture {
        DragGesture()
            .onChanged { value in
                offset = CGSize(
                    width: dragOrigin.width + value.translation.width,
                    height: dragOrigin.height + value.translation.height
                )
            }
            .onEnded { _ in dragOrigin = offset }
    }

    private func iconButton(
        _ symbol: String,
        help: String,
        disabled: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .frame(width: 18, height: 18)
        }
        .buttonStyle(.borderless)
        .disabled(disabled)
        .help(help)
    }

    private func adjacent(_ delta: Int) -> String? {
        guard let id = model.viewerAssetID else { return nil }
        return model.adjacentAsset(from: id, delta: delta)
    }

    private func navigate(_ delta: Int) {
        if let next = adjacent(delta) { model.viewerAssetID = next }
    }

    private func zoom(_ factor: Double) {
        withAnimation(.easeOut(duration: 0.16)) {
            scale = max(0.08, min(10, scale * factor))
        }
    }

    private func resetTransform() {
        withAnimation(.easeOut(duration: 0.2)) {
            scale = 1
            gestureScale = 1
            offset = .zero
            dragOrigin = .zero
        }
    }

    private func close() {
        model.viewerAssetID = nil
        dismiss()
    }

    private func loadCurrentImage() async {
        guard let asset = currentAsset, let runDirectory = model.payload?.runDir else { return }
        loading = true
        failed = false
        resetTransform()
        let result = await Task.detached(priority: .userInitiated) { () -> NSImage? in
            do {
                let preview = try bridge.preparePreview(
                    runDirectory: runDirectory,
                    assetID: asset.id,
                    maxLongEdge: 6144
                )
                return NSImage(contentsOfFile: preview.path)
            } catch {
                return nil
            }
        }.value
        guard model.viewerAssetID == asset.id else { return }
        image = result
        loading = false
        failed = result == nil
    }
}
