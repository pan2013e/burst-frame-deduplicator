import BurstFrameAppCore
import Testing

@Test
func rustBridgeDefaultOptions() throws {
    let options = try RustBridge().defaultOptions()
    #expect(options.previewSize == 1280)
    #expect(options.refineSize == 2048)
    #expect(options.acceleration == "auto")
}

@Test @MainActor
func externalLocaleCatalogLoadsBothLanguages() {
    let catalog = LocaleCatalog()
    #expect(catalog.loadError == nil)
    catalog.code = "en"
    #expect(catalog.text("keep") == "Keep")
    catalog.code = "zh-CN"
    #expect(catalog.text("keep") == "保留")
}
