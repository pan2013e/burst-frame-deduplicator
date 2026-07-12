import BurstFrameAppCore
import Testing

@Test
func rustBridgeDefaultOptions() throws {
    let bridge = RustBridge()
    #expect(bridge.apiVersion == 4)
    let options = try bridge.defaultOptions()
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
    #expect(catalog.text("counterpartCard") == "Counterpart Card")
    catalog.code = "zh-CN"
    #expect(catalog.text("keep") == "保留")
    #expect(catalog.text("counterpartCard") == "对应格式存储卡")
}

@Test
func tutorialProgressRecordsCompletionAndSkip() {
    var encodedRecord: String?
    var legacyCompleted = false
    let finishedAt = "2026-07-12T00:00:00Z"
    let store = TutorialProgressStore(
        readRecord: { encodedRecord },
        writeRecord: { encodedRecord = $0 },
        legacyCompleted: { legacyCompleted },
        markLegacyComplete: { legacyCompleted = true },
        now: { finishedAt }
    )
    #expect(!store.hasFinished())

    store.record(.skipped)
    #expect(store.hasFinished())
    #expect(store.currentRecord() == TutorialProgressRecord(outcome: .skipped, finishedAt: finishedAt))

    store.record(.completed)
    #expect(store.currentRecord()?.outcome == .completed)
}

@Test
func tutorialProgressMigratesLegacyCompletion() {
    var encodedRecord: String?
    var legacyCompleted = true
    let store = TutorialProgressStore(
        readRecord: { encodedRecord },
        writeRecord: { encodedRecord = $0 },
        legacyCompleted: { legacyCompleted },
        markLegacyComplete: { legacyCompleted = true },
        now: { "2026-07-12T00:00:00Z" }
    )
    #expect(store.hasFinished())
    #expect(store.currentRecord()?.outcome == .completed)
    #expect(encodedRecord != nil)
}

@Test
func imageMagnificationFitsInsidePhysicalViewport() {
    let plan = ImageViewportGeometry.magnificationPlan(
        imageWidth: 6_000,
        imageHeight: 4_000,
        viewportWidth: 1_200,
        viewportHeight: 800
    )
    #expect(abs(plan.fit - 0.188) < 0.000_001)
    #expect(plan.minimum < plan.fit)
    #expect(plan.maximum > plan.fit)
}

@Test
func imageMagnificationDoesNotUpscaleSmallImages() {
    let plan = ImageViewportGeometry.magnificationPlan(
        imageWidth: 400,
        imageHeight: 300,
        viewportWidth: 1_200,
        viewportHeight: 900
    )
    #expect(plan.fit == 1)
}

@Test
func embeddedPreviewSkipsRefinementWhenItCoversRetinaViewport() {
    let needsRefinement = ImageViewportGeometry.previewNeedsRefinement(
        availableWidth: 1_920,
        availableHeight: 1_440,
        magnification: 0.46,
        backingScale: 2,
        targetLongEdge: 4_096
    )
    #expect(!needsRefinement)
}

@Test
func embeddedPreviewRefinesWhenZoomExceedsItsDeviceResolution() {
    let needsRefinement = ImageViewportGeometry.previewNeedsRefinement(
        availableWidth: 1_920,
        availableHeight: 1_440,
        magnification: 0.60,
        backingScale: 2,
        targetLongEdge: 4_096
    )
    #expect(needsRefinement)
}

@Test
func nearTargetEmbeddedPreviewDoesNotRenderForMarginalGain() {
    let needsRefinement = ImageViewportGeometry.previewNeedsRefinement(
        availableWidth: 3_840,
        availableHeight: 2_560,
        magnification: 1,
        backingScale: 2,
        targetLongEdge: 4_096
    )
    #expect(!needsRefinement)
}

@Test
func refinedPreviewSwapPreservesDisplayedScale() {
    let magnification = ImageViewportGeometry.replacementMagnification(
        oldMagnification: 0.60,
        oldImageWidth: 1_920,
        newImageWidth: 4_096,
        minimumMagnification: 0.01,
        maximumMagnification: 8
    )
    #expect(abs(magnification - 0.28125) < 0.000_001)
    #expect(abs(1_920 * 0.60 - 4_096 * magnification) < 0.000_001)
}
