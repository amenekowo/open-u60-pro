import SwiftUI

struct DeviceControlView: View {
    @Bindable var viewModel: DeviceControlViewModel

    var body: some View {
        List {
            if let msg = viewModel.message {
                Section {
                    Text(msg)
                        .font(.subheadline)
                        .foregroundStyle(viewModel.messageIsError ? .red : .green)
                }
            }

            Section {
                Toggle("Power Supply", isOn: Binding(
                    get: { viewModel.powerSupplyEnabled },
                    set: { val in
                        viewModel.powerSupplyEnabled = val
                        Task { await viewModel.setPowerSupply(enabled: val) }
                    }
                ))
                    .disabled(viewModel.isLoading)
            } footer: {
                Text("When enabled, the device runs directly from the AC adapter and maintains battery at 40–60% to extend battery lifespan.")
            }

            Section {
                Toggle("Power-save Mode", isOn: Binding(
                    get: { viewModel.powerSaveEnabled },
                    set: { val in
                        viewModel.powerSaveEnabled = val
                        Task { await viewModel.setPowerSave(enabled: val) }
                    }
                ))
                    .disabled(viewModel.isLoading)
            } footer: {
                Text("Restricts data communication speed to reduce consumption and extend battery life.")
            }

            Section {
                Toggle("Fast Boot", isOn: Binding(
                    get: { viewModel.fastBootEnabled },
                    set: { val in
                        viewModel.fastBootEnabled = val
                        Task { await viewModel.setFastBoot(enabled: val) }
                    }
                ))
                    .disabled(viewModel.isLoading)
            } footer: {
                Text("When enabled, powering off suspends to RAM for near-instant boot. Disabling uses full shutdown (saves battery when off).")
            }

            Section {
                Button("Reboot Router") {
                    viewModel.showRebootConfirm = true
                }
                .disabled(viewModel.isLoading)
            } footer: {
                Text("The router will restart. This takes about 60 seconds.")
            }

            Section {
                Button("Factory Reset", role: .destructive) {
                    viewModel.showFactoryResetConfirm = true
                }
                .disabled(viewModel.isLoading)
            } footer: {
                Text("This will erase all settings and restore factory defaults. This cannot be undone.")
            }
        }
        .task { await viewModel.refresh() }
        .navigationTitle("Device Controls")
        .overlay {
            if viewModel.isLoading {
                ProgressView()
                    .padding()
                    .background(Color(.systemBackground).opacity(0.85), in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .sheet(isPresented: $viewModel.showRebootConfirm) {
            PasswordConfirmView(
                title: "Reboot Router",
                message: "Enter your router password to confirm reboot.",
                confirmLabel: "Reboot"
            ) {
                await viewModel.reboot()
            }
        }
        .sheet(isPresented: $viewModel.showFactoryResetConfirm) {
            PasswordConfirmView(
                title: "Factory Reset",
                message: "This will erase ALL settings. Enter your router password to confirm.",
                confirmLabel: "Factory Reset"
            ) {
                await viewModel.factoryReset()
            }
        }
    }
}
