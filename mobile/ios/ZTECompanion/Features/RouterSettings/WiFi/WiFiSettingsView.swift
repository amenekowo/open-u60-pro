import SwiftUI

struct WiFiSettingsView: View {
    @Bindable var viewModel: WiFiSettingsViewModel

    var body: some View {
        List {
            if let msg = viewModel.message {
                Section {
                    Text(msg)
                        .font(.subheadline)
                        .foregroundStyle(viewModel.messageIsError ? .red : .green)
                }
            }

            Section("2.4 GHz") {
                TextField("SSID", text: $viewModel.editSSID2g)
                    .autocorrectionDisabled()
                SecureField("Password", text: $viewModel.editKey2g)

                Picker("Channel", selection: $viewModel.editChannel2g) {
                    ForEach(WiFiConfig.channelOptions2g, id: \.self) { ch in
                        Text(ch == "auto" ? "Auto" : "Ch \(ch)").tag(ch)
                    }
                }

                Picker("TX Power", selection: $viewModel.editTxpower2g) {
                    ForEach(WiFiConfig.txpowerOptions, id: \.self) { pwr in
                        Text("\(pwr)%").tag(pwr)
                    }
                }

                Picker("Encryption", selection: $viewModel.editEncryption2g) {
                    ForEach(WiFiConfig.encryptionOptions, id: \.self) { enc in
                        Text(encryptionLabel(enc)).tag(enc)
                    }
                }

                Toggle("Hidden SSID", isOn: $viewModel.editHidden2g)
            }

            Section("5 GHz") {
                TextField("SSID", text: $viewModel.editSSID5g)
                    .autocorrectionDisabled()
                SecureField("Password", text: $viewModel.editKey5g)

                Picker("Channel", selection: $viewModel.editChannel5g) {
                    ForEach(WiFiConfig.channelOptions5g, id: \.self) { ch in
                        Text(ch == "auto" ? "Auto" : "Ch \(ch)").tag(ch)
                    }
                }

                Picker("TX Power", selection: $viewModel.editTxpower5g) {
                    ForEach(WiFiConfig.txpowerOptions, id: \.self) { pwr in
                        Text("\(pwr)%").tag(pwr)
                    }
                }

                Picker("Encryption", selection: $viewModel.editEncryption5g) {
                    ForEach(WiFiConfig.encryptionOptions, id: \.self) { enc in
                        Text(encryptionLabel(enc)).tag(enc)
                    }
                }

                Toggle("Hidden SSID", isOn: $viewModel.editHidden5g)
            }

            Section {
                Button {
                    Task { await viewModel.apply() }
                } label: {
                    Text("Apply")
                        .frame(maxWidth: .infinity)
                }
                .disabled(viewModel.isLoading)
            }
        }
        .navigationTitle("WiFi Settings")
        .refreshable { await viewModel.refresh() }
        .overlay {
            if viewModel.isLoading {
                ProgressView()
                    .padding()
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .task { await viewModel.refresh() }
    }

    private func encryptionLabel(_ enc: String) -> String {
        switch enc {
        case "none": return "None"
        case "psk+tkip": return "WPA-PSK (TKIP)"
        case "psk+ccmp": return "WPA-PSK (AES)"
        case "psk2+ccmp": return "WPA2-PSK (AES)"
        case "psk-mixed+ccmp": return "WPA/WPA2 Mixed"
        default: return enc
        }
    }
}
