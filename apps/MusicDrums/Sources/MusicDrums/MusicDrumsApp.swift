import AppKit
import SwiftUI

@main
struct MusicDrumsApp: App {
    @StateObject private var appState = AppState()

    var body: some Scene {
        MenuBarExtra("Music Drums", systemImage: appState.menuIcon) {
            VStack(alignment: .leading, spacing: 10) {
                Text("MX Master 4 Music Drums")
                    .font(.headline)

                Toggle("Drums mode", isOn: $appState.drumsEnabled)
                    .toggleStyle(.switch)

                HStack {
                    Text("Sensitivity")
                    Slider(value: $appState.sensitivity, in: 0.1 ... 1.0, step: 0.05)
                        .frame(width: 140)
                    Text(String(format: "%.2f", appState.sensitivity))
                        .monospacedDigit()
                        .frame(width: 36, alignment: .trailing)
                }

                Divider()

                LabeledContent("Link") {
                    Text(appState.linkLabel)
                }
                LabeledContent("Hits") {
                    Text("\(appState.hitsFired)")
                }
                LabeledContent("Options+") {
                    Text(appState.optionsParked ? "Parked" : "Active / unknown")
                }

                if let err = appState.lastError, !err.isEmpty {
                    Text(err)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .lineLimit(4)
                }

                Divider()

                Button("Test pulse") {
                    appState.testPulse()
                }
                .disabled(!appState.drumsEnabled)

                Button("Refresh status") {
                    appState.refreshStatus()
                }

                Divider()

                Button("Quit") {
                    appState.shutdown()
                    NSApplication.shared.terminate(nil)
                }
            }
            .padding(12)
            .frame(width: 280)
        }
        .menuBarExtraStyle(.window)
    }
}
