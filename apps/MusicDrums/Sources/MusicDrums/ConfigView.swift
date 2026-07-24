import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct ConfigView: View {
    @ObservedObject var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var draft = DrumsConfigModel.classic()
    @State private var presets: [PresetInfoModel] = []
    @State private var selectedPresetId: String = "classic"
    @State private var message: String?
    @State private var suppressPresetLoad = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Detection & presets")
                    .font(.headline)
                Spacer()
                Button("Done") { dismiss() }
                    .keyboardShortcut(.cancelAction)
            }

            Picker("Preset", selection: $selectedPresetId) {
                ForEach(presets) { p in
                    Text(p.builtin ? "\(p.name) (built-in)" : p.name)
                        .tag(p.id)
                }
            }
            .onChange(of: selectedPresetId) { _, id in
                guard !suppressPresetLoad else { return }
                loadPreset(id)
            }

            if !draft.description.isEmpty {
                Text(draft.description)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            GroupBox("Feel") {
                VStack(alignment: .leading, spacing: 8) {
                    sliderRow("Sensitivity", value: $draft.sensitivity, range: 0.1 ... 1.0, step: 0.05)
                    Picker("Detector", selection: $draft.detector.mode) {
                        Text("Classic multiband").tag("multiband")
                        Text("Kick / house").tag("kick")
                    }
                    Toggle("Drop hats", isOn: $draft.mapper.dropHats)
                    Toggle("Compound kick pulses", isOn: $draft.mapper.useCompoundKick)
                }
                .padding(.vertical, 4)
            }

            GroupBox("Timing") {
                VStack(alignment: .leading, spacing: 8) {
                    sliderRow("Cooldown ms", value: $draft.detector.cooldownMs, range: 40 ... 250, step: 5)
                    sliderRow("Min interval ms", value: $draft.mapper.minIntervalMs, range: 40 ... 250, step: 5)
                    sliderRow(
                        "Drain frames",
                        value: intAsDouble($draft.engine.drainFrames),
                        range: 256 ... 8192,
                        step: 256
                    )
                    sliderRow(
                        "Idle sleep ms",
                        value: u64AsDouble($draft.engine.idleSleepMs),
                        range: 0 ... 20,
                        step: 1
                    )
                }
                .padding(.vertical, 4)
            }

            GroupBox("Gates") {
                VStack(alignment: .leading, spacing: 8) {
                    sliderRow("Flux base", value: $draft.detector.fluxBase, range: 0.0002 ... 0.01, step: 0.0002)
                    sliderRow("Flux mean ×", value: $draft.detector.fluxMeanMul, range: 1 ... 6, step: 0.1)
                    sliderRow("Bass ratio min", value: $draft.detector.bassRatioMin, range: 0 ... 1.2, step: 0.05)
                    sliderRow("Min strength", value: $draft.mapper.minStrengthBase, range: 0.02 ... 0.4, step: 0.01)
                }
                .padding(.vertical, 4)
            }

            GroupBox("Identity") {
                VStack(alignment: .leading, spacing: 8) {
                    TextField("Name", text: $draft.name)
                    TextField("Id (filename slug)", text: $draft.id)
                }
                .padding(.vertical, 4)
            }

            if let message {
                Text(message)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Button("Apply") { applyDraft() }
                    .keyboardShortcut(.defaultAction)
                Button("Save preset…") { savePreset() }
                Button("Import…") { importPanel() }
                Button("Export…") { exportPanel() }
                Spacer()
                Button("Reset Classic") {
                    suppressPresetLoad = true
                    draft = .classic()
                    selectedPresetId = "classic"
                    suppressPresetLoad = false
                    applyDraft()
                }
            }
        }
        .padding(14)
        .frame(width: 440)
        .onAppear {
            refreshPresets()
            suppressPresetLoad = true
            draft = appState.loadConfigModel() ?? .classic()
            selectedPresetId = draft.id
            suppressPresetLoad = false
        }
    }

    private func sliderRow(
        _ title: String,
        value: Binding<Double>,
        range: ClosedRange<Double>,
        step: Double
    ) -> some View {
        // SwiftUI crashes if step does not fit the range (Normalizing precondition).
        let safeStep = sanitizedStep(range: range, step: step)
        return HStack {
            Text(title)
                .frame(width: 120, alignment: .leading)
            Slider(value: value, in: range, step: safeStep)
            Text(format(value.wrappedValue, step: safeStep))
                .monospacedDigit()
                .frame(width: 56, alignment: .trailing)
        }
    }

    private func sanitizedStep(range: ClosedRange<Double>, step: Double) -> Double {
        let span = range.upperBound - range.lowerBound
        guard span > 0 else { return 1 }
        var s = step
        if s <= 0 || s > span {
            s = span / 20
        }
        // Ensure at least one full stride fits.
        if span / s < 1 {
            s = span
        }
        return s
    }

    private func format(_ v: Double, step: Double) -> String {
        if step < 0.001 { return String(format: "%.4f", v) }
        if step < 0.01 { return String(format: "%.3f", v) }
        if step < 1 { return String(format: "%.2f", v) }
        return String(format: "%.0f", v)
    }

    private func intAsDouble(_ int: Binding<Int>) -> Binding<Double> {
        Binding(
            get: { Double(int.wrappedValue) },
            set: { int.wrappedValue = Int($0.rounded()) }
        )
    }

    private func u64AsDouble(_ int: Binding<UInt64>) -> Binding<Double> {
        Binding(
            get: { Double(int.wrappedValue) },
            set: { int.wrappedValue = UInt64($0.rounded()) }
        )
    }

    private func refreshPresets() {
        presets = appState.listPresets()
        if presets.isEmpty {
            presets = [
                PresetInfoModel(id: "classic", name: "Classic", description: "", builtin: true),
                PresetInfoModel(id: "house", name: "House / Electronic", description: "", builtin: true),
            ]
        }
    }

    private func loadPreset(_ id: String) {
        if let cfg = appState.loadPreset(id) {
            draft = cfg
            message = "Loaded \(cfg.name)"
        } else {
            message = appState.lastError ?? "Failed to load preset"
        }
    }

    private func applyDraft() {
        if let err = appState.applyConfig(draft) {
            message = err
        } else {
            message = "Applied \(draft.name)"
            appState.sensitivity = draft.sensitivity
            refreshPresets()
        }
    }

    private func savePreset() {
        if let err = appState.applyConfig(draft) {
            message = err
            return
        }
        if let err = appState.saveCurrentPreset() {
            message = err
        } else {
            message = "Saved to Application Support presets/"
            refreshPresets()
            suppressPresetLoad = true
            selectedPresetId = draft.id
            suppressPresetLoad = false
        }
    }

    private func importPanel() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if let err = appState.importConfig(path: url.path) {
            message = err
        } else if let cfg = appState.loadConfigModel() {
            suppressPresetLoad = true
            draft = cfg
            selectedPresetId = cfg.id
            suppressPresetLoad = false
            appState.sensitivity = cfg.sensitivity
            message = "Imported \(cfg.name)"
            refreshPresets()
        }
    }

    private func exportPanel() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "\(draft.id).json"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do {
            try draft.toJSONString().write(to: url, atomically: true, encoding: .utf8)
            message = "Exported \(url.lastPathComponent)"
        } catch {
            message = error.localizedDescription
        }
    }
}
