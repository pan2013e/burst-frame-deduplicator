import AppKit
import BurstFrameAppCore

enum DirectoryPanelPurpose {
    case photos
    case run
    case results
    case moveDestination

    var titleKey: String {
        switch self {
        case .photos: "selectPhotosTitle"
        case .run: "selectRunTitle"
        case .results: "selectResultsTitle"
        case .moveDestination: "selectMoveDestinationTitle"
        }
    }

    var messageKey: String {
        switch self {
        case .photos: "selectPhotosMessage"
        case .run: "selectRunMessage"
        case .results: "selectResultsMessage"
        case .moveDestination: "selectMoveDestinationMessage"
        }
    }
}

@MainActor
func chooseDirectory(
    for purpose: DirectoryPanelPurpose,
    locale: LocaleCatalog,
    startingAt directory: URL? = nil
) -> URL? {
    let panel = NSOpenPanel()
    panel.title = locale.text(purpose.titleKey)
    panel.message = locale.text(purpose.messageKey)
    panel.prompt = locale.text("selectFolder")
    panel.canChooseDirectories = true
    panel.canChooseFiles = false
    panel.allowsMultipleSelection = false
    panel.canCreateDirectories = purpose == .results || purpose == .moveDestination
    panel.directoryURL = directory
    return panel.runModal() == .OK ? panel.url?.standardizedFileURL : nil
}
