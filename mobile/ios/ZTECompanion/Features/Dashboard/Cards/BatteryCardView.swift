import SwiftUI

struct BatteryCardView: View {
    let battery: BatteryStatus

    var body: some View {
        CardView {
            VStack(spacing: 8) {
                Image(systemName: batteryIcon(battery.capacity))
                    .font(.title2)
                    .foregroundStyle(Color.batteryColor(battery.capacity))
                AnimatedNumber(value: battery.capacity,
                               font: .title3.weight(.bold), textColor: .primary, suffix: "%")
                batteryStatusLine
                if battery.temperature > 0 {
                    AnimatedNumber(value: battery.temperature, decimalPlaces: 0,
                                   font: .caption, textColor: .secondary, suffix: "\u{00B0}C")
                }
                Text("Battery")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
        }
    }

    @ViewBuilder
    private var batteryStatusLine: some View {
        HStack(spacing: 4) {
            batteryStatusText
            if let ma = battery.currentMA {
                Text("\u{00B7}").font(.caption).foregroundStyle(.secondary)
                if let mv = battery.voltageMV {
                    let watts = Double(mv) * Double(abs(ma)) / 1_000_000.0
                    AnimatedNumber(value: watts, decimalPlaces: 1,
                                   font: .caption.monospacedDigit(),
                                   textColor: batteryStatusColor,
                                   suffix: "W")
                } else {
                    AnimatedNumber(value: ma,
                                   font: .caption.monospacedDigit(),
                                   textColor: batteryStatusColor,
                                   prefix: ma >= 0 ? "+" : nil,
                                   suffix: "mA")
                }
            }
        }
        .lineLimit(1)
        .minimumScaleFactor(0.8)
    }

    @ViewBuilder
    private var batteryStatusText: some View {
        switch battery.charging {
        case "wall":
            Text("Wall Mode")
                .font(.caption)
                .foregroundStyle(.blue)
        case "charging":
            if battery.capacity >= 100 {
                Text("Full")
                    .font(.caption)
                    .foregroundStyle(.green)
            } else if battery.currentMA != nil {
                Text("Charging")
                    .font(.caption)
                    .foregroundStyle(.green)
            } else if battery.timeToFull > 0 {
                Text("Charging \u{00B7} \(formatETA(battery.timeToFull))")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.green)
            } else {
                Text("Charging")
                    .font(.caption)
                    .foregroundStyle(.green)
            }
        default:
            if battery.timeToEmpty > 0 {
                if battery.currentMA != nil {
                    Text(formatETA(battery.timeToEmpty))
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(Color.batteryColor(battery.capacity))
                } else {
                    Text("\(formatETA(battery.timeToEmpty)) left")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(Color.batteryColor(battery.capacity))
                }
            } else {
                Text("Discharging")
                    .font(.caption)
                    .foregroundStyle(Color.batteryColor(battery.capacity))
            }
        }
    }

    private var batteryStatusColor: Color {
        switch battery.charging {
        case "wall": return .blue
        case "charging": return .green
        default: return Color.batteryColor(battery.capacity)
        }
    }

    private func formatETA(_ minutes: Int) -> String {
        if minutes >= 1440 {
            let d = minutes / 1440, h = (minutes % 1440) / 60, m = minutes % 60
            return "\(d)d \(h)h \(m)m"
        }
        return minutes >= 60 ? "\(minutes / 60)h \(minutes % 60)m" : "\(minutes)m"
    }

    private func batteryIcon(_ percent: Int) -> String {
        if percent >= 75 { return "battery.100" }
        if percent >= 50 { return "battery.75" }
        if percent >= 25 { return "battery.50" }
        return "battery.25"
    }
}
