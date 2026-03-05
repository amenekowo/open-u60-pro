import SwiftUI

struct WiFiCardView: View {
    let wifiStatus: WifiStatus

    var body: some View {
        CardView {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("WiFi")
                        .font(.headline)
                    if wifiStatus.wifiOn && wifiStatus.wifi6 {
                        Text("WiFi 7")
                            .font(.caption2.bold())
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(.blue.opacity(0.15), in: Capsule())
                            .foregroundStyle(.blue)
                    }
                    if wifiStatus.wifiOn && wifiStatus.clientsTotal > 0 {
                        HStack(spacing: 2) {
                            AnimatedNumber(value: wifiStatus.clientsTotal,
                                           font: .caption2, textColor: .secondary)
                            Text("client\(wifiStatus.clientsTotal == 1 ? "" : "s")")
                                .font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    Spacer()
                    Text(wifiStatus.wifiOn ? "On" : "Off")
                        .font(.caption)
                        .foregroundStyle(wifiStatus.wifiOn ? .green : .red)
                }

                if wifiStatus.wifiOn {
                    wifiBandRow(
                        label: "2.4G",
                        disabled: wifiStatus.radio2gDisabled,
                        ssid: wifiStatus.ssid2g,
                        hidden: wifiStatus.hidden2g,
                        encryption: wifiStatus.encryption2g,
                        channel: wifiStatus.channel2g,
                        txPower: wifiStatus.txPower2g,
                        bandwidth: wifiStatus.bandwidth2g
                    )
                    wifiBandRow(
                        label: "5G",
                        disabled: wifiStatus.radio5gDisabled,
                        ssid: wifiStatus.ssid5g,
                        hidden: wifiStatus.hidden5g,
                        encryption: wifiStatus.encryption5g,
                        channel: wifiStatus.channel5g,
                        txPower: wifiStatus.txPower5g,
                        bandwidth: wifiStatus.bandwidth5g
                    )
                }
            }
        }
    }

    private func wifiBandRow(
        label: String, disabled: Bool, ssid: String, hidden: Bool,
        encryption: String, channel: String, txPower: String,
        bandwidth: String
    ) -> some View {
        HStack {
            Label(label, systemImage: "wifi")
                .font(.caption)
            Spacer()
            if disabled {
                Text("Disabled")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                if hidden {
                    Text("(Hidden)")
                        .font(.caption2)
                        .foregroundStyle(.orange)
                }
                if !ssid.isEmpty {
                    Text(ssid)
                        .font(.caption.bold())
                        .lineLimit(1)
                }
                if !encryption.isEmpty {
                    Text(encryption)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                if !channel.isEmpty {
                    Text("CH \(channel)")
                        .font(.caption2.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
                if let bwLabel = formatBandwidth(bandwidth) {
                    Text(bwLabel)
                        .font(.caption2.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
                if !txPower.isEmpty {
                    Text("TX \(txPower)%")
                        .font(.caption2.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private func formatBandwidth(_ htmode: String) -> String? {
        let mode = htmode.uppercased()
        guard !mode.isEmpty, mode != "AUTO" else { return nil }
        // Extract width from EHT160, HE80, VHT40, HT20, etc.
        if let range = mode.range(of: "\\d+$", options: .regularExpression) {
            return mode[range] + "M"
        }
        return nil
    }
}
