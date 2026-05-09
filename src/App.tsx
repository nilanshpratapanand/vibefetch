import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type DownloadProgressPayload = {
  percentage: number;
  speed?: string | null;
  eta?: string | null;
  status: string;
  done?: boolean;
  success?: boolean;
};

type DeepLinkPayload = {
  url?: string;
};

function App() {
  const [url, setUrl] = useState("");
  const [startTime, setStartTime] = useState("");
  const [endTime, setEndTime] = useState("");
  const [status, setStatus] = useState("Ready.");
  const [isLoading, setIsLoading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [speed, setSpeed] = useState("--");
  const [eta, setEta] = useState("--");
  const [isSuccess, setIsSuccess] = useState(false);
  const [errorLog, setErrorLog] = useState("");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let unlistenDeepLink: (() => void) | undefined;

    async function setupListener() {
      unlisten = await listen<DownloadProgressPayload>(
        "download-progress",
        (event) => {
          const payload = event.payload;
          if (!payload) return;
          setProgress(Math.max(0, Math.min(100, payload.percentage ?? 0)));
          if (payload.speed) setSpeed(payload.speed);
          if (payload.eta) setEta(payload.eta);
          if (payload.status) {
            setStatus(payload.status);
          }
          if (payload.done) {
            setIsLoading(false);
            setIsSuccess(Boolean(payload.success));
          }
        },
      );

      unlistenDeepLink = await listen<DeepLinkPayload>(
        "deep-link-received",
        (event) => {
          const deepLinkUrl = String(event.payload?.url ?? "").trim();
        if (!deepLinkUrl) return;
        setUrl(deepLinkUrl);
        setStatus("Deep link received. Starting Fetch Info...");
        fetch_formats(deepLinkUrl);
        },
      );
    }

    setupListener();

    return () => {
      if (unlisten) unlisten();
      if (unlistenDeepLink) unlistenDeepLink();
    };
  }, []);

  async function handleDownload(overrideUrl?: string) {
    const finalUrl = (overrideUrl ?? url).trim();
    if (!finalUrl) {
      setStatus("Please enter a URL first.");
      return;
    }

    setIsLoading(true);
    setIsSuccess(false);
    setProgress(0);
    setSpeed("--");
    setEta("--");
    setErrorLog("");
    setStatus("Starting download...");

    try {
      const result = await invoke<string>("download_with_engine", {
        url: finalUrl,
        start_time: startTime.trim() || null,
        end_time: endTime.trim() || null,
      });
      setProgress(100);
      setEta("0s");
      setIsSuccess(true);
      setStatus(result || "Download completed.");
    } catch (error) {
      const message = String(error);
      console.error("Download failed:", error);
      setIsSuccess(false);
      setStatus(`Download failed: ${message}`);
      setErrorLog(message);
    } finally {
      setIsLoading(false);
    }
  }

  async function fetch_formats(deepLinkUrl?: string) {
    await handleDownload(deepLinkUrl);
  }

  async function handleOpenDownloads() {
    try {
      await invoke("open_downloads");
    } catch (error) {
      setStatus(`Could not open Downloads: ${String(error)}`);
    }
  }

  return (
    <main className="app-root">
      <section className="app-card">
        <h1 className="app-title">VibeFetch Downloader</h1>
        <p className="app-subtitle">
          Paste a URL and run your Python engine from Tauri.
        </p>

      <form
        className="app-form"
        onSubmit={(e) => {
          e.preventDefault();
          handleDownload();
        }}
      >
        <input
          type="url"
          value={url}
          onChange={(e) => setUrl(e.currentTarget.value)}
          placeholder="https://example.com/video"
          className="app-input"
          disabled={isLoading}
        />
        <button
          type="submit"
          className="app-button"
          disabled={isLoading}
        >
          {isLoading ? "Downloading..." : "Download"}
        </button>
      </form>
      <div className="clip-row">
        <input
          type="text"
          value={startTime}
          onChange={(e) => setStartTime(e.currentTarget.value)}
          placeholder="Start Time (00:00:10)"
          className="app-input clip-input"
          disabled={isLoading}
        />
        <input
          type="text"
          value={endTime}
          onChange={(e) => setEndTime(e.currentTarget.value)}
          placeholder="End Time (00:00:40)"
          className="app-input clip-input"
          disabled={isLoading}
        />
      </div>

        <div className="app-status">
          <div className="progress-header">
            <span>Progress</span>
            <span>{progress}%</span>
          </div>
          <div
            className="progress-track"
            role="progressbar"
            aria-valuenow={progress}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div className="progress-fill" style={{ width: `${progress}%` }} />
          </div>
          <div className="stats-row">
            <div className="stat-chip">
              <span className="stat-label">Speed</span>
              <span className="stat-value">{speed}</span>
            </div>
            <div className="stat-chip">
              <span className="stat-label">ETA</span>
              <span className="stat-value">{eta}</span>
            </div>
          </div>
          <div className="status-text">
            {status}
          </div>
          {errorLog && <div className="error-log">Error: {errorLog}</div>}
          {isSuccess && (
            <button
              type="button"
              className="open-downloads-button"
              onClick={handleOpenDownloads}
            >
              Open Downloads
            </button>
          )}
        </div>
      </section>
    </main>
  );
}

export default App;
