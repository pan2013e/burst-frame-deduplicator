import AppKit
import SwiftUI

struct TriStateCheckbox: NSViewRepresentable {
    let state: FrameDecision
    let accessibilityLabel: String
    let onChange: (FrameDecision) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeNSView(context: Context) -> NSButton {
        let button = NSButton(checkboxWithTitle: "", target: context.coordinator, action: #selector(Coordinator.changed(_:)))
        button.allowsMixedState = true
        button.controlSize = .regular
        button.setAccessibilityLabel(accessibilityLabel)
        return button
    }

    func updateNSView(_ button: NSButton, context: Context) {
        context.coordinator.parent = self
        button.setAccessibilityLabel(accessibilityLabel)
        button.state = switch state {
        case .keep: .on
        case .reject: .off
        case .review: .mixed
        }
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSButton, context: Context) -> CGSize? {
        nsView.fittingSize
    }

    final class Coordinator: NSObject {
        var parent: TriStateCheckbox

        init(parent: TriStateCheckbox) {
            self.parent = parent
        }

        @objc func changed(_ sender: NSButton) {
            let decision: FrameDecision = switch sender.state {
            case .on: .keep
            case .off: .reject
            default: .review
            }
            parent.onChange(decision)
        }
    }
}
