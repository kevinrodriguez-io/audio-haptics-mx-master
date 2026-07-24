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

                Toggle("Drums mode", isOn: appState.drumsModeBinding)
                    .toggleStyle(.switch)
                    .disabled(appState.isChangingMode)

                HStack {
                    Text("Sensitivity")
                    Slider(value: $appState.sensitivity, in: 0.1 ... 1.0, step: 0.05)
                        .frame(width: 140)
                    Text(String(format: "%.2f", appState.sensitivity))
                        .monospacedDigit()
                        .frame(width: 36, alignment: .trailing)
                }

                Divider()

                LabeledContent("Preset") {
                    Text(appState.presetName)
                }
                LabeledContent("Link") {
                    Text(appState.linkLabel)
                }
                LabeledContent("Hits") {
                    Text("\(appState.hitsFired)")
                }
                LabeledContent("Options+") {
                    Text(appState.optionsParked ? "Parked" : "Active / unknown")
                }

                InputMeterView(
                    samples: appState.waveform,
                    level: appState.inputLevel,
                    framesReceived: appState.audioFramesReceived,
                    isLive: appState.drumsEnabled && appState.tapRunning
                )

                if appState.isChangingMode {
                    Text("Starting…")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                if let err = appState.lastError, !err.isEmpty {
                    Text(err)
                        .font(.caption)
                        .foregroundStyle(.red)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: 260, alignment: .leading)

                    if err.contains("0xE00002E2")
                        || err.localizedCaseInsensitiveContains("not permitted")
                        || err.localizedCaseInsensitiveContains("Input Monitoring")
                    {
                        Button("Open Input Monitoring…") {
                            HidAccess.openSystemSettings()
                        }
                    }
                }

                Divider()

                Button("Configure…") {
                    appState.showConfig = true
                }
                .popover(isPresented: $appState.showConfig, arrowEdge: .bottom) {
                    ConfigView(appState: appState)
                }

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
            .frame(width: 300)
        }
        .menuBarExtraStyle(.window)
    }
}
