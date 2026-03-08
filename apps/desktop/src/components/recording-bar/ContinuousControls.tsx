import { invoke } from "@tauri-apps/api/core";
import { Waveform } from "./Waveform";

// Buttons fade out faster (100ms) than the pill shrinks (300ms)
// so they disappear before overflowing the pill boundary.
const BUTTON_FADE = "opacity 100ms ease, transform 100ms ease";

export function ContinuousControls({ showSideButtons = true }: { showSideButtons?: boolean }) {
  const handleCancel = () => {
    invoke("bar_cancel_recording");
  };

  const handleStop = () => {
    invoke("bar_stop_recording");
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        width: "100%",
        padding: "0 14px",
        gap: 8,
      }}
    >
      {/* Cancel button */}
      <button
        onClick={handleCancel}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: 26,
          height: 26,
          borderRadius: "var(--radius-md)",
          background: "rgba(255, 255, 255, 0.08)",
          border: "1px solid var(--color-border)",
          cursor: "pointer",
          flexShrink: 0,
          padding: 0,
          opacity: showSideButtons ? 1 : 0,
          pointerEvents: showSideButtons ? "auto" : "none",
          transition: BUTTON_FADE,
        }}
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 10 10"
          fill="none"
          stroke="rgba(255, 255, 255, 0.4)"
          strokeWidth="1.5"
          strokeLinecap="round"
          style={{ pointerEvents: "none" }}
        >
          <path d="M1 1L9 9M9 1L1 9" />
        </svg>
      </button>

      {/* Waveform */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <Waveform barCount={14} />
      </div>

      {/* Stop button */}
      <button
        onClick={handleStop}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: 26,
          height: 26,
          borderRadius: "var(--radius-md)",
          background: "var(--color-danger)",
          border: "none",
          boxShadow: showSideButtons ? "0 0 12px var(--color-danger-glow)" : "none",
          transform: "scale(1)",
          cursor: "pointer",
          flexShrink: 0,
          padding: 0,
          opacity: showSideButtons ? 1 : 0,
          pointerEvents: showSideButtons ? "auto" : "none",
          transition: BUTTON_FADE,
        }}
      >
        <div
          style={{
            width: 8,
            height: 8,
            borderRadius: 2,
            background: "var(--color-text)",
            pointerEvents: "none",
          }}
        />
      </button>
    </div>
  );
}
