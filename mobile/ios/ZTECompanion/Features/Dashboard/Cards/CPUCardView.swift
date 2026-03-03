import SwiftUI

struct CPUCardView: View {
    let systemInfo: SystemInfo
    let thermal: ThermalStatus

    var body: some View {
        CardView {
            VStack(spacing: 8) {
                Image(systemName: "cpu")
                    .font(.title2)
                    .foregroundStyle(
                        systemInfo.cpuUsagePercent > 0
                            ? Color.cpuUsageColor(systemInfo.cpuUsagePercent)
                            : (thermal.cpuTemp > 70 ? .red : .orange)
                    )
                if systemInfo.cpuUsagePercent > 0 {
                    AnimatedNumber(value: systemInfo.cpuUsagePercent, decimalPlaces: 0,
                                   font: .title3.weight(.bold),
                                   textColor: Color.cpuUsageColor(systemInfo.cpuUsagePercent),
                                   suffix: "%")
                } else {
                    Text("--")
                        .font(.title3.monospacedDigit().bold())
                        .foregroundStyle(.secondary)
                }
                AnimatedNumber(value: thermal.cpuTemp, decimalPlaces: 0,
                               font: .caption, textColor: .secondary, suffix: "\u{00B0}C")
                Text("CPU")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
        }
    }
}
