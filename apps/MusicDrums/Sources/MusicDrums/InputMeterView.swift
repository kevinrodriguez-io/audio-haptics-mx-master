import SwiftUI

/// Simple scrolling waveform / level strip for verifying Process Tap input.
struct InputMeterView: View {
    var samples: [Float]
    var level: Double
    var framesReceived: UInt64
    var isLive: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text("Input")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Text(statusLabel)
                    .font(.caption2.monospacedDigit())
                    .foregroundStyle(levelColor)
            }

            Canvas { context, size in
                let midY = size.height * 0.5
                let n = max(samples.count, 1)
                let step = size.width / CGFloat(n)
                var path = Path()
                for (i, s) in samples.enumerated() {
                    let x = CGFloat(i) * step + step * 0.5
                    let amp = CGFloat(min(abs(s), 1.0)) * (size.height * 0.45)
                    if i == 0 {
                        path.move(to: CGPoint(x: x, y: midY - amp))
                    } else {
                        path.addLine(to: CGPoint(x: x, y: midY - amp))
                    }
                }
                for (i, s) in samples.enumerated().reversed() {
                    let x = CGFloat(i) * step + step * 0.5
                    let amp = CGFloat(min(abs(s), 1.0)) * (size.height * 0.45)
                    path.addLine(to: CGPoint(x: x, y: midY + amp))
                }
                path.closeSubpath()
                context.fill(path, with: .color(levelColor.opacity(0.85)))

                // Level bar on the right edge.
                let barW: CGFloat = 4
                let barH = size.height * CGFloat(min(level, 1.0))
                let barRect = CGRect(
                    x: size.width - barW,
                    y: size.height - barH,
                    width: barW,
                    height: barH
                )
                context.fill(Path(barRect), with: .color(levelColor))
            }
            .frame(height: 44)
            .background(Color.primary.opacity(0.06))
            .clipShape(RoundedRectangle(cornerRadius: 4))

            if isLive && level < 0.002 && framesReceived > 8_000 {
                Text("Tap is silent — play system audio, or toggle Drums off/on to rebuild the tap.")
                    .font(.caption2)
                    .foregroundStyle(.orange)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var statusLabel: String {
        if !isLive { return "idle" }
        if framesReceived == 0 { return "waiting…" }
        return String(format: "lvl %.0f%% · %llu f", level * 100, framesReceived)
    }

    private var levelColor: Color {
        if !isLive { return .secondary }
        if level < 0.002 { return .orange }
        if level > 0.35 { return .green }
        return .accentColor
    }
}
