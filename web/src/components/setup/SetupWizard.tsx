"use client";

import { useState, useCallback } from "react";
import { enableADB } from "@/lib/ubus";
import { ADBClient } from "@/lib/adb";
import { CopyButton } from "@/components/ui/CopyButton";
import { DeployLog, type LogEntry } from "./DeployLog";
import {
  Wifi,
  Usb,
  Loader2,
  Check,
  AlertCircle,
} from "lucide-react";
import { cn } from "@/lib/utils";

const RELEASE_URL =
  "https://github.com/jesther-ai/open-u60-pro/releases/latest/download/zte-agent";

const STEP_NAMES = [
  "Connect",
  "Credentials",
  "Enabling",
  "USB",
  "Deploying",
  "Success",
];

export function SetupWizard() {
  const [step, setStep] = useState(0);
  const [password, setPassword] = useState("");
  const [agentPassword, setAgentPassword] = useState("");
  const [gateway, setGateway] = useState("192.168.0.1");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);

  const addLog = useCallback(
    (text: string, state: LogEntry["state"] = "active") => {
      setLogEntries((prev) => [...prev, { text, state }]);
    },
    []
  );

  const finishLastLog = useCallback(() => {
    setLogEntries((prev) => {
      const next = [...prev];
      const last = next.findLastIndex((e) => e.state === "active");
      if (last >= 0) next[last] = { ...next[last], state: "done" };
      return next;
    });
  }, []);

  const errorLastLog = useCallback(() => {
    setLogEntries((prev) => {
      const next = [...prev];
      const last = next.findLastIndex((e) => e.state === "active");
      if (last >= 0) next[last] = { ...next[last], state: "error" };
      return next;
    });
  }, []);

  const handleEnableADB = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (!password) {
      setError("Please enter your router password.");
      return;
    }
    if (!agentPassword) {
      setError("Please enter a password for the agent API.");
      return;
    }

    setLoading(true);
    setStep(2);

    try {
      await enableADB(gateway, password);
      setStep(3);
    } catch (err) {
      setStep(1);
      setError(
        err instanceof Error
          ? err.message
          : "Connection failed. Ensure you are on the router's network."
      );
    } finally {
      setLoading(false);
    }
  };

  const handleConnectUSB = async () => {
    setError("");

    if (typeof navigator === "undefined" || !navigator.usb) {
      setError(
        "WebUSB is not supported in this browser. Please use Chrome or Edge."
      );
      return;
    }

    try {
      const adb = new ADBClient();
      await adb.connect();
      setStep(4);
      setLogEntries([]);

      // Download binary
      addLog("Downloading zte-agent binary...");
      const response = await fetch(RELEASE_URL);
      if (!response.ok) throw new Error(`Download failed: HTTP ${response.status}`);
      const binary = new Uint8Array(await response.arrayBuffer());
      finishLastLog();
      addLog(
        `Downloaded ${(binary.length / 1024 / 1024).toFixed(1)} MB`,
        "done"
      );

      // Push binary
      addLog("Pushing binary to device...");
      await adb.push(binary, "/data/zte-agent", 33261);
      finishLastLog();

      // Create boot script
      addLog("Creating boot script...");
      const escapedPassword = agentPassword.replace(/'/g, "'\\''");
      const script = `#!/bin/sh\nexport ZTE_AGENT_PASSWORD='${escapedPassword}'\n/data/zte-agent >/dev/null 2>&1 &\n`;
      await adb.shell(
        "cat > /data/local/tmp/start_zte_agent.sh << 'BOOTEOF'\n" +
          script +
          "BOOTEOF"
      );
      await adb.shell("chmod +x /data/local/tmp/start_zte_agent.sh");
      finishLastLog();

      // Configure auto-start
      addLog("Configuring auto-start...");
      await adb.shell(
        "grep -q start_zte_agent /etc/rc.local || sed -i '/^exit 0/i sh /data/local/tmp/start_zte_agent.sh' /etc/rc.local"
      );
      finishLastLog();

      // Start agent
      addLog("Starting agent...");
      await adb.shell("sh /data/local/tmp/start_zte_agent.sh");
      finishLastLog();

      // Verify
      addLog("Verifying agent is running...");
      await new Promise((r) => setTimeout(r, 3000));
      const verifyRes = await fetch(
        `http://${gateway}:9090/api/auth/login`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ password: agentPassword }),
        }
      );
      if (!verifyRes.ok) throw new Error("Agent not responding after start");
      finishLastLog();
      addLog("Agent is running!", "done");

      setStep(5);
    } catch (err) {
      if (err instanceof Error && err.name === "NotFoundError") return;

      if (step === 4) {
        errorLastLog();
        setError(
          err instanceof Error ? err.message : "Deployment failed."
        );
      } else {
        setError(
          err instanceof Error
            ? err.message
            : "Failed to connect USB device."
        );
      }
    }
  };

  const endpointUrl = `http://${gateway}:9090`;

  return (
    <div className="mx-auto max-w-md">
      {/* Step dots */}
      <div className="mb-5 flex gap-2">
        {STEP_NAMES.map((_, i) => (
          <div
            key={i}
            className={cn(
              "h-2 w-2 rounded-full transition-colors",
              i === step
                ? "bg-accent"
                : i < step
                  ? "bg-success"
                  : "bg-border"
            )}
          />
        ))}
      </div>

      {/* Step 0: Connect */}
      {step === 0 && (
        <div>
          <h2 className="mb-2 text-lg font-semibold">Connect to Your Router</h2>
          <p className="mb-4 text-sm text-text-dim">
            Connect to your U60 Pro&apos;s WiFi network (or use the default
            Ethernet connection).
          </p>
          <button
            onClick={() => setStep(1)}
            className="w-full rounded-lg bg-accent py-2.5 text-sm font-semibold text-white transition-colors hover:bg-accent-hover"
          >
            I&apos;m Connected
          </button>
        </div>
      )}

      {/* Step 1: Credentials */}
      {step === 1 && (
        <div>
          <h2 className="mb-2 text-lg font-semibold">Enter Credentials</h2>
          <p className="mb-4 text-sm text-text-dim">
            Router admin password to enable ADB, and a password for the
            zte-agent API.
          </p>
          <form onSubmit={handleEnableADB} className="space-y-3">
            <div>
              <label className="mb-1.5 block text-[0.8125rem] font-medium">
                Router Password
              </label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Admin password"
                className="w-full rounded-lg border border-border bg-bg-input px-3 py-2.5 text-sm text-text outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-accent/25"
                autoComplete="off"
              />
            </div>
            <div>
              <label className="mb-1.5 block text-[0.8125rem] font-medium">
                Agent Password
              </label>
              <input
                type="password"
                value={agentPassword}
                onChange={(e) => setAgentPassword(e.target.value)}
                placeholder="Password for zte-agent API"
                className="w-full rounded-lg border border-border bg-bg-input px-3 py-2.5 text-sm text-text outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-accent/25"
                autoComplete="off"
              />
            </div>
            <div>
              <label className="mb-1.5 block text-[0.8125rem] font-medium">
                Gateway IP
              </label>
              <input
                type="text"
                value={gateway}
                onChange={(e) => setGateway(e.target.value)}
                className="w-full rounded-lg border border-border bg-bg-input px-3 py-2.5 text-sm text-text outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-accent/25"
                autoComplete="off"
              />
            </div>
            <button
              type="submit"
              disabled={loading}
              className="w-full rounded-lg bg-accent py-2.5 text-sm font-semibold text-white transition-colors hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Enable ADB
            </button>
          </form>
          {error && (
            <div className="mt-3 flex items-start gap-2 rounded-lg border border-error/30 bg-error/10 p-3 text-[0.8125rem] text-error">
              <AlertCircle size={16} className="mt-0.5 shrink-0" />
              {error}
            </div>
          )}
        </div>
      )}

      {/* Step 2: Enabling */}
      {step === 2 && (
        <div className="text-center">
          <h2 className="mb-2 text-lg font-semibold">Enabling ADB...</h2>
          <p className="mb-4 text-sm text-text-dim">
            Authenticating with your router and switching USB mode. This takes
            a few seconds.
          </p>
          <div className="flex justify-center py-4">
            <Loader2 size={24} className="animate-spin text-accent" />
          </div>
        </div>
      )}

      {/* Step 3: USB */}
      {step === 3 && (
        <div>
          <h2 className="mb-2 text-lg font-semibold">Connect USB Cable</h2>
          <p className="mb-4 text-sm text-text-dim">
            ADB is enabled. Now connect your U60 Pro to this computer with a
            USB-C cable, then click below.
          </p>
          <div className="mb-4 flex flex-col items-center py-4">
            <Usb size={48} className="mb-3 text-text-dim" />
            <span className="text-center text-[0.8125rem] text-text-dim">
              A browser prompt will ask you to select the USB device.
            </span>
          </div>
          <button
            onClick={handleConnectUSB}
            className="w-full rounded-lg bg-accent py-2.5 text-sm font-semibold text-white transition-colors hover:bg-accent-hover"
          >
            Connect Device
          </button>
          {error && (
            <div className="mt-3 flex items-start gap-2 rounded-lg border border-error/30 bg-error/10 p-3 text-[0.8125rem] text-error">
              <AlertCircle size={16} className="mt-0.5 shrink-0" />
              {error}
            </div>
          )}
          {typeof navigator !== "undefined" && !navigator.usb && (
            <div className="mt-3 rounded-lg border border-accent/20 bg-accent/5 p-3 text-[0.8125rem] text-text-dim">
              WebUSB is not supported in this browser. Please use Chrome or
              Edge on desktop.
            </div>
          )}
        </div>
      )}

      {/* Step 4: Deploying */}
      {step === 4 && (
        <div>
          <h2 className="mb-2 text-lg font-semibold">Deploying Agent...</h2>
          <p className="mb-1 text-sm text-text-dim">
            Downloading, pushing, and starting zte-agent on your router.
          </p>
          <DeployLog entries={logEntries} />
          {error && (
            <div className="mt-3 flex items-start gap-2 rounded-lg border border-error/30 bg-error/10 p-3 text-[0.8125rem] text-error">
              <AlertCircle size={16} className="mt-0.5 shrink-0" />
              {error}
            </div>
          )}
        </div>
      )}

      {/* Step 5: Success */}
      {step === 5 && (
        <div>
          <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-success/15">
            <Check size={24} className="text-success" />
          </div>
          <h2 className="mb-2 text-lg font-semibold">Setup Complete!</h2>
          <p className="mb-4 text-sm text-text-dim">
            The zte-agent is running on your router and will auto-start on
            reboot.
          </p>
          <hr className="my-4 border-border" />
          <p className="mb-2 text-sm font-semibold">Next steps:</p>
          <p className="text-sm text-text-dim">
            1. Download the companion app for{" "}
            <a
              href="https://github.com/jesther-ai/open-u60-pro/tree/main/mobile/ios"
              target="_blank"
              rel="noopener noreferrer"
              className="text-accent hover:underline"
            >
              iOS
            </a>{" "}
            or{" "}
            <a
              href="https://github.com/jesther-ai/open-u60-pro/tree/main/mobile/android"
              target="_blank"
              rel="noopener noreferrer"
              className="text-accent hover:underline"
            >
              Android
            </a>
            .
          </p>
          <p className="text-sm text-text-dim">
            2. Connect to your router&apos;s WiFi and open the app.
          </p>
          <p className="text-sm text-text-dim">
            3. Log in with the agent password you set above.
          </p>
          <hr className="my-4 border-border" />
          <p className="mb-1 text-[0.8125rem] text-text-dim">
            Agent API endpoint:
          </p>
          <div className="flex items-center justify-between rounded-lg border border-border bg-bg px-3 py-2.5">
            <code className="font-mono text-xs text-text">
              {endpointUrl}
            </code>
            <CopyButton text={endpointUrl} />
          </div>
        </div>
      )}
    </div>
  );
}
