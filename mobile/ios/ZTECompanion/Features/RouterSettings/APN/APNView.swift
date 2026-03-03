import SwiftUI

struct APNView: View {
    @Bindable var viewModel: APNViewModel

    var body: some View {
        List {
            if let msg = viewModel.message {
                Section {
                    Text(msg)
                        .font(.subheadline)
                        .foregroundStyle(viewModel.messageIsError ? .red : .green)
                }
            }

            Section("APN Mode") {
                Toggle("Manual APN", isOn: Binding(
                    get: { viewModel.config.isManual },
                    set: { manual in Task { await viewModel.setMode(manual: manual) } }
                ))
                .disabled(viewModel.isLoading)
            }

            if viewModel.config.isManual {
                Section {
                    Button {
                        viewModel.newProfile = .empty
                        viewModel.showAddSheet = true
                    } label: {
                        Label("Add APN", systemImage: "plus")
                    }
                } header: {
                    Text("Profiles")
                }

                if !viewModel.config.profiles.isEmpty {
                    Section {
                        ForEach(viewModel.config.profiles) { profile in
                            VStack(alignment: .leading, spacing: 4) {
                                HStack {
                                    Text(profile.name.isEmpty ? "Unnamed" : profile.name)
                                        .font(.headline)
                                    Spacer()
                                    if profile.active {
                                        Text("Active")
                                            .font(.caption)
                                            .foregroundStyle(.green)
                                    }
                                }
                                Text(profile.apn)
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                Text("\(profile.pdpType) / \(profile.authMode)")
                                    .font(.caption)
                                    .foregroundStyle(.tertiary)
                            }
                            .padding(.vertical, 2)
                            .swipeActions(edge: .trailing) {
                                Button(role: .destructive) {
                                    Task { await viewModel.deleteAPN(profile) }
                                } label: {
                                    Label("Delete", systemImage: "trash")
                                }
                            }
                            .swipeActions(edge: .leading) {
                                if !profile.active {
                                    Button {
                                        Task { await viewModel.activateAPN(profile) }
                                    } label: {
                                        Label("Activate", systemImage: "checkmark.circle")
                                    }
                                    .tint(.green)
                                }
                            }
                        }
                    }
                }
            }
        }
        .navigationTitle("APN Settings")
        .refreshable { await viewModel.refresh() }
        .overlay {
            if viewModel.isLoading {
                ProgressView()
                    .padding()
                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8))
            }
        }
        .task { await viewModel.refresh() }
        .sheet(isPresented: $viewModel.showAddSheet) {
            APNFormSheet(viewModel: viewModel)
        }
    }
}

struct APNFormSheet: View {
    @Bindable var viewModel: APNViewModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                Section("Profile") {
                    TextField("Name", text: $viewModel.newProfile.name)
                    TextField("APN", text: $viewModel.newProfile.apn)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                }

                Section("Connection") {
                    Picker("PDP Type", selection: $viewModel.newProfile.pdpType) {
                        ForEach(APNProfile.pdpTypeOptions, id: \.self) { type in
                            Text(type).tag(type)
                        }
                    }

                    Picker("Auth Mode", selection: $viewModel.newProfile.authMode) {
                        ForEach(APNProfile.authModeOptions, id: \.self) { mode in
                            Text(mode).tag(mode)
                        }
                    }
                }

                if viewModel.newProfile.authMode != "none" {
                    Section("Credentials") {
                        TextField("Username", text: $viewModel.newProfile.username)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                        SecureField("Password", text: $viewModel.newProfile.password)
                    }
                }

                Section {
                    Button {
                        Task { await viewModel.addAPN() }
                    } label: {
                        Text("Add APN")
                            .frame(maxWidth: .infinity)
                    }
                    .disabled(viewModel.newProfile.name.isEmpty || viewModel.newProfile.apn.isEmpty || viewModel.isLoading)
                }
            }
            .navigationTitle("New APN")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}
