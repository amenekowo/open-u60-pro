import SwiftUI

struct DNSSettingsView: View {
    @Bindable var viewModel: DNSSettingsViewModel

    var body: some View {
        List {
            if let msg = viewModel.message {
                Section {
                    Text(msg)
                        .font(.subheadline)
                        .foregroundStyle(viewModel.messageIsError ? .red : .green)
                }
            }

            Section("Current DNS") {
                LabeledContent("Mode", value: viewModel.config.wanDnsMode.isEmpty ? "—" : viewModel.config.wanDnsMode)
                if viewModel.config.isManual {
                    LabeledContent("Primary", value: viewModel.config.primaryDns)
                    LabeledContent("Secondary", value: viewModel.config.secondaryDns)
                }
            }

            if !viewModel.config.ipv6DnsMode.isEmpty {
                Section("IPv6 DNS") {
                    LabeledContent("Mode", value: viewModel.config.ipv6DnsMode)
                    if !viewModel.config.ipv6PrimaryDns.isEmpty {
                        LabeledContent("Primary", value: viewModel.config.ipv6PrimaryDns)
                    }
                    if !viewModel.config.ipv6SecondaryDns.isEmpty {
                        LabeledContent("Secondary", value: viewModel.config.ipv6SecondaryDns)
                    }
                }
            }

            Section("Configure") {
                Toggle("Manual DNS", isOn: $viewModel.editMode)

                if viewModel.editMode {
                    TextField("Primary DNS", text: $viewModel.editPrimary)
                        .keyboardType(.decimalPad)
                        .textContentType(.URL)
                        .autocorrectionDisabled()

                    TextField("Secondary DNS", text: $viewModel.editSecondary)
                        .keyboardType(.decimalPad)
                        .textContentType(.URL)
                        .autocorrectionDisabled()
                }

                Button {
                    Task { await viewModel.applyDNS() }
                } label: {
                    Text("Apply")
                        .frame(maxWidth: .infinity)
                }
                .disabled(viewModel.isLoading)
            }

            Section {
                Button("Use Cloudflare (1.1.1.1)") {
                    viewModel.editMode = true
                    viewModel.editPrimary = "1.1.1.1"
                    viewModel.editSecondary = "1.0.0.1"
                }
                Button("Use Google (8.8.8.8)") {
                    viewModel.editMode = true
                    viewModel.editPrimary = "8.8.8.8"
                    viewModel.editSecondary = "8.8.4.4"
                }
            } header: {
                Text("Presets")
            }
        }
        .navigationTitle("DNS Settings")
        .refreshable { await viewModel.refresh() }
        .overlay {
            if viewModel.isLoading {
                ProgressView()
                    .padding()
                    .background(Color(.systemBackground).opacity(0.85), in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .task { await viewModel.refresh() }
    }
}
