import AppKit
import SwiftUI

private let thumbnailCache: NSCache<NSString, NSImage> = {
    let cache = NSCache<NSString, NSImage>()
    cache.countLimit = 320
    cache.totalCostLimit = 192 * 1024 * 1024
    return cache
}()

struct CachedImageView: View {
    let path: String?

    var body: some View {
        if let image = loadImage() {
            Image(nsImage: image)
                .resizable()
                .scaledToFill()
        } else {
            ZStack {
                Rectangle().fill(.quaternary)
                Image(systemName: "photo")
                    .font(.title2)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func loadImage() -> NSImage? {
        guard let path else { return nil }
        let key = path as NSString
        if let cached = thumbnailCache.object(forKey: key) { return cached }
        guard let image = NSImage(contentsOfFile: path) else { return nil }
        thumbnailCache.setObject(image, forKey: key, cost: imageCost(image))
        return image
    }

    private func imageCost(_ image: NSImage) -> Int {
        guard let representation = image.representations.first else { return 0 }
        return representation.pixelsWide * representation.pixelsHigh * 4
    }
}
