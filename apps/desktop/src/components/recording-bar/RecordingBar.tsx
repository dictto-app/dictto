import { useState, useEffect, useRef } from "react";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { LogicalSize, LogicalPosition } from "@tauri-apps/api/dpi";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useRecording } from "../../hooks/useRecording";
import { ContinuousControls } from "./ContinuousControls";
import { StatusIndicator } from "./StatusIndicator";

interface RecordingBarProps {
  barVisibleIdle: boolean;
  barOpacity: number;
}

const BOTTOM_OFFSET = 60;

// Extra padding around measured content to prevent box-shadow clipping.
// Max shadow is `0 4px 24px` → blur 24px + offset 4px = 28px.
// Extra buffer covers tooltip hover animation overshoot (~8px translateY),
// sub-pixel rounding, and DPI scaling margins.
const SHADOW_PADDING = 50;

// Padding around the idle pill hitbox. Zero = hitbox matches visible pill exactly,
// eliminating the dead zone where WebView2 captures events but CSS ignores them.
const IDLE_HITBOX_PADDING = 0;

// Pill dimensions per Figma spec
const PILL = {
  idle: { w: 48, h: 7 },
  idleHover: { w: 56, h: 28 },
  recordingPTT: { w: 130, h: 42 },
  toggle: { w: 192, h: 42 },
  processing: { w: 130, h: 42 },
} as const;

// Pre-computed tooltip dimensions for the idle hover tooltip.
// Text: "Click or hold Ctrl + Win to start dictating" at fontSize:13, padding:10px 20px.
// These constants prevent the first-hover window resize flicker caused by
// non-atomic setSize+setPosition in Tauri (see CLAUDE.md).
const TOOLTIP = {
  idle: { w: 320, h: 42 },    // Measured tooltip width+height (with small buffer)
  gap: 10,                      // Space between pill bottom edge and tooltip bottom
  arrowOverhang: 5,             // Arrow tip extends below tooltip
} as const;

// CSS transition applied to the pill at ALL times (not just idle).
// This ensures smooth animation for every state change.
const PILL_TRANSITION = [
  "width 300ms cubic-bezier(0.2, 0.8, 0.2, 1)",
  "height 300ms cubic-bezier(0.2, 0.8, 0.2, 1)",
  "background 250ms ease",
  "border-color 250ms ease",
  "box-shadow 350ms ease",
  "backdrop-filter 250ms ease",
].join(", ");

// Content layer fade duration
const CONTENT_FADE_MS = 200;
const CONTENT_TRANSITION = `opacity ${CONTENT_FADE_MS}ms ease`;

type VisualState =
  | "hidden"
  | "idle"
  | "idleHover"
  | "recordingPTT"
  | "toggle"
  | "toggleHover"
  | "processing";

// Collapse hover variants so window only resizes on group change, not on hover
type SizeGroup = "hidden" | "idle" | "recordingPTT" | "toggle" | "processing";

function toSizeGroup(vs: VisualState): SizeGroup {
  switch (vs) {
    case "hidden":
      return "hidden";
    case "idle":
    case "idleHover":
      return "idle";
    case "recordingPTT":
      return "recordingPTT";
    case "toggle":
    case "toggleHover":
      return "toggle";
    case "processing":
      return "processing";
  }
}

/** Get the smallest window size that fully contains the pill for each group.
 *  Smaller window = less WebView2 area interfering with caret blink in editors.
 */
function getGroupPillSize(group: SizeGroup): { w: number; h: number } {
  switch (group) {
    case "hidden":
      return { w: 1, h: 1 };
    case "idle":
      // Include tooltip so hover doesn't trigger a window resize
      return {
        w: Math.max(PILL.idleHover.w, TOOLTIP.idle.w),
        h: PILL.idleHover.h + TOOLTIP.gap + TOOLTIP.idle.h,
      };
    case "recordingPTT":
      return PILL.recordingPTT;
    case "toggle":
      return PILL.toggle;
    case "processing":
      return PILL.processing;
  }
}

interface MonitorInfo {
  logicalX: number;
  logicalY: number;
  logicalW: number;
  logicalH: number;
  scaleFactor: number;
}


export function RecordingBar({ barVisibleIdle, barOpacity }: RecordingBarProps) {
  const { recordingState, recordingMode } = useRecording();
  const [isHovered, setIsHovered] = useState(false);
  const [isToggleHovered, setIsToggleHovered] = useState(false);
  const [monitorInfo, setMonitorInfo] = useState<MonitorInfo | null>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const lastSizeRef = useRef<{ group: SizeGroup; w: number; h: number } | null>(null);

  const isActive = recordingState !== "idle";
  const isContinuous =
    recordingState === "recording" && recordingMode === "continuous";
  const showPill = isActive || barVisibleIdle;

  // Compute visual state
  const visualState: VisualState = (() => {
    if (!showPill) return "hidden";
    if (recordingState === "processing") return "processing";
    if (recordingState === "recording") {
      if (isContinuous) return isToggleHovered ? "toggleHover" : "toggle";
      return "recordingPTT";
    }
    return isHovered ? "idleHover" : "idle";
  })();

  // Reset hover flags when leaving their relevant state group
  useEffect(() => {
    if (recordingState !== "idle") setIsHovered(false);
    if (!isContinuous) setIsToggleHovered(false);
  }, [recordingState, isContinuous]);

  // Get monitor info + force webview transparency on mount
  useEffect(() => {
    currentMonitor().then((mon) => {
      if (mon) {
        setMonitorInfo({
          logicalX: mon.position.x / mon.scaleFactor,
          logicalY: mon.position.y / mon.scaleFactor,
          logicalW: mon.size.width / mon.scaleFactor,
          logicalH: mon.size.height / mon.scaleFactor,
          scaleFactor: mon.scaleFactor,
        });
      }
    });
    getCurrentWebview()
      .setBackgroundColor({ red: 0, green: 0, blue: 0, alpha: 0 })
      .catch(() => {});
  }, []);

  // --- Window sizing ---
  // Two modes:
  //   1. Group change (idle→recording, etc.): precompute from PILL constants.
  //      Use MAX(old, new) during the CSS transition, then settle after 350ms.
  //   2. Hover within group (idle→idleHover): measure DOM to capture tooltip,
  //      only grow (never shrink on unhover → prevents flicker).
  useEffect(() => {
    if (!monitorInfo) return;

    const win = getCurrentWindow();
    const group = toSizeGroup(visualState);

    if (group === "hidden") {
      lastSizeRef.current = null;
      win.setSize(new LogicalSize(1, 1));
      return;
    }

    const applySize = (w: number, h: number) => {
      lastSizeRef.current = { group, w, h };
      const x = monitorInfo.logicalX + (monitorInfo.logicalW - w) / 2;
      const y = monitorInfo.logicalY + monitorInfo.logicalH - h - BOTTOM_OFFSET + SHADOW_PADDING;
      Promise.all([
        win.setSize(new LogicalSize(w, h)),
        win.setPosition(new LogicalPosition(x, y)),
      ]);
    };

    const isGroupChange = !lastSizeRef.current || lastSizeRef.current.group !== group;

    if (isGroupChange) {
      // Pre-size window for the new group using PILL constants.
      // Use MAX(old, new) so the pill is never clipped during the CSS transition.
      // We intentionally do NOT shrink later — transparent areas are click-through
      // on Windows, and shrinking causes a visible position jump because setSize
      // and setPosition are not atomic in Tauri.
      const targetPill = getGroupPillSize(group);
      let w = targetPill.w + SHADOW_PADDING * 2;
      let h = targetPill.h + SHADOW_PADDING * 2;

      if (lastSizeRef.current) {
        w = Math.max(w, lastSizeRef.current.w);
        h = Math.max(h, lastSizeRef.current.h);
      }

      applySize(w, h);
      return;
    }

    // No resize on hover or within-group changes — window is already at max size.
    // Transparent areas are click-through, so oversized window has zero UX cost.
  }, [visualState, monitorInfo]);

  // --- Hitbox sync: tell Rust where the pill is in physical screen pixels ---
  // Uses actual window position from Tauri (same coordinate space as WH_MOUSE_LL hook)
  // instead of recomputing from monitor coordinates, to avoid DPI rounding discrepancies.
  useEffect(() => {
    if (!monitorInfo) return;

    const group = toSizeGroup(visualState);

    if (group === "hidden") {
      invoke("clear_pill_hitbox");
      return;
    }

    // Idle (no hover) gets small padding for hover targeting comfort.
    // All other states: padding=0 (hitbox matches pill exactly, no dead zone).
    const isIdleNoHover = visualState === "idle";
    const padding = isIdleNoHover ? IDLE_HITBOX_PADDING : 0;

    // Target PILL dimensions per visual state (avoids measuring mid-transition CSS).
    const targetPill: { w: number; h: number } = (() => {
      switch (visualState) {
        case "idle": return PILL.idle;
        case "idleHover": return PILL.idleHover;
        case "toggle":
        case "toggleHover": return PILL.toggle;
        case "recordingPTT": return PILL.recordingPTT;
        case "processing": return PILL.processing;
        default: return PILL.idle;
      }
    })();

    let cancelled = false;

    const sendHitbox = async () => {
      try {
        const win = getCurrentWindow();
        const pos = await win.innerPosition();
        if (cancelled) return;

        const dpr = window.devicePixelRatio;
        const paddingPhys = padding * dpr;

        // Compute pill position from layout constants — no getBoundingClientRect.
        // Layout: flex container with alignItems:flex-end (bottom pinned),
        // justifyContent:center, paddingBottom:SHADOW_PADDING.
        // This is deterministic and immune to CSS transition timing.
        const vpW = window.innerWidth;   // CSS viewport (stable, no transition)
        const vpH = window.innerHeight;
        const pillLeft = (vpW - targetPill.w) / 2;
        const pillRight = (vpW + targetPill.w) / 2;
        const pillBottom = vpH - SHADOW_PADDING;
        const pillTop = pillBottom - targetPill.h;

        invoke("update_pill_hitbox", {
          left: Math.floor(pos.x + pillLeft * dpr - paddingPhys),
          top: Math.floor(pos.y + pillTop * dpr - paddingPhys),
          right: Math.ceil(pos.x + pillRight * dpr + paddingPhys),
          bottom: Math.ceil(pos.y + pillBottom * dpr + paddingPhys),
        });
      } catch {
        // Window might not be ready yet; hitbox will update on next state change
      }
    };

    // Hover-within-group: 50ms (window already sized, just need innerPosition settled).
    // Group changes: 350ms (window resize + CSS transition must complete first so
    // innerPosition and window.innerWidth/Height reflect the settled state).
    const isHoverState = visualState === "idleHover" || visualState === "toggleHover";
    const delay = isHoverState ? 50 : 350;
    const timer = setTimeout(sendHitbox, delay);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [visualState, monitorInfo]);

  // --- Listen for cursor enter/leave events from Rust mouse hook ---
  // Window is always click-through, so DOM mouse events don't fire.
  // Hover state is driven entirely by the WH_MOUSE_LL hook.
  useEffect(() => {
    const unlistenEnter = listen("pill-cursor-entered", () => {
      if (recordingState === "idle") setIsHovered(true);
      if (recordingState === "recording") setIsToggleHovered(true);
    });
    const unlistenLeave = listen("pill-cursor-left", () => {
      setIsHovered(false);
      setIsToggleHovered(false);
    });
    return () => {
      unlistenEnter.then((f) => f());
      unlistenLeave.then((f) => f());
    };
  }, [recordingState]);

  // --- Listen for hitbox-click event from Rust mouse hook ---
  // The WH_MOUSE_LL hook consumes the real click to prevent WebView2 from
  // stealing focus. We receive the physical screen coordinates here and
  // dispatch a synthetic click on the element under the cursor.
  useEffect(() => {
    const unlisten = listen<{ screenX: number; screenY: number }>("hitbox-click", async (event) => {
      try {
        const win = getCurrentWindow();
        const innerPos = await win.innerPosition();
        const dpr = window.devicePixelRatio || 1;
        // Convert physical screen coords → CSS viewport coords
        const cssX = (event.payload.screenX - innerPos.x) / dpr;
        const cssY = (event.payload.screenY - innerPos.y) / dpr;
        const el = document.elementFromPoint(cssX, cssY);
        if (el instanceof HTMLElement) {
          // Find the closest clickable ancestor (button, [data-click], or
          // element with onClick). This handles clicks on SVG paths, inner
          // divs, etc. that are children of the actual clickable button.
          const clickable = el.closest("button, [data-click]") as HTMLElement | null;
          const target = clickable ?? el;

          // Press animation (Apple/Google style — feedback on click, not hover)
          target.classList.remove("btn-press");
          void target.offsetWidth; // force reflow to restart animation
          target.classList.add("btn-press");
          target.addEventListener(
            "animationend",
            () => target.classList.remove("btn-press"),
            { once: true },
          );

          target.click();
        }
      } catch {
        // Fallback: ignore coordinate errors silently
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  if (!showPill) return null;

  // --- Pill styles ---
  const isIdle = recordingState === "idle";
  const pillHovered = isIdle ? isHovered : isToggleHovered;

  // Determine pill dimensions
  let pillW: number, pillH: number;
  if (isIdle) {
    pillW = pillHovered ? PILL.idleHover.w : PILL.idle.w;
    pillH = pillHovered ? PILL.idleHover.h : PILL.idle.h;
  } else if (recordingState === "processing") {
    pillW = PILL.processing.w;
    pillH = PILL.processing.h;
  } else if (isContinuous) {
    pillW = PILL.toggle.w;
    pillH = PILL.toggle.h;
  } else {
    pillW = PILL.recordingPTT.w;
    pillH = PILL.recordingPTT.h;
  }

  // Pill visual styles per Figma HTML spec
  const pillBackground = isIdle
    ? "var(--color-surface-blur)"
    : "var(--color-bg)";

  const pillBorder = (() => {
    if (isIdle && !pillHovered) return "1px solid var(--color-border)";
    if (isIdle && pillHovered) return "1px solid var(--color-border-strong)";
    return "1px solid var(--color-border)";
  })();

  const pillShadow = (() => {
    if (isIdle && !pillHovered) return "none";
    return "0 4px 24px rgba(0,0,0,0.5)";
  })();

  // Tooltip visibility
  const showIdleTooltip = isIdle && isHovered;
  const showToggleTooltip = isContinuous && isToggleHovered;

  // Content layer visibility
  const showRecording = visualState === "recordingPTT" || visualState === "toggle" || visualState === "toggleHover";
  const showProcessing = visualState === "processing";

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "flex-end",
        justifyContent: "center",
        paddingBottom: SHADOW_PADDING,
        opacity: isActive ? 1 : barOpacity,
        pointerEvents: "none",
        userSelect: "none",
      }}
    >
      {/* Measurement wrapper — captures pill + absolute tooltips */}
      <div ref={measureRef}>
      {/* The pill — always-on CSS transitions */}
      <div
        onClick={() => {
          if (isIdle) invoke("bar_start_recording");
        }}
        style={{
          width: pillW,
          height: pillH,
          background: pillBackground,
          borderRadius: 100,
          border: pillBorder,
          boxShadow: pillShadow,
          backdropFilter: isIdle ? "blur(12px)" : "none",
          WebkitBackdropFilter: isIdle ? "blur(12px)" : "none",
          position: "relative",
          overflow: "visible",
          boxSizing: "border-box",
          cursor: isIdle ? "pointer" : "default",
          pointerEvents: "auto",
          transition: PILL_TRANSITION,
        }}
      >
        {/* Tooltip — idle hover (absolutely positioned above pill) */}
        {isIdle && (
          <div
            style={{
              position: "absolute",
              bottom: "calc(100% + 10px)",
              left: "50%",
              transform: showIdleTooltip
                ? "translateX(-50%) translateY(0)"
                : "translateX(-50%) translateY(8px)",
              opacity: showIdleTooltip ? 1 : 0,
              pointerEvents: "none",
              transition:
                "opacity 200ms ease, transform 250ms cubic-bezier(0.2, 0.8, 0.2, 1)",
              transitionDelay: showIdleTooltip ? "100ms" : "0ms",
              whiteSpace: "nowrap",
              background: "var(--color-surface)",
              border: "1px solid rgba(255,255,255,0.1)",
              borderRadius: 100,
              padding: "10px 20px",
              fontSize: 13,
              color: "var(--color-text-secondary)",
              boxShadow: "0 4px 20px rgba(0,0,0,0.4)",
            }}
          >
            Click or hold <kbd style={{ fontWeight: 700, color: "var(--color-text)", fontSize: 13, fontFamily: "inherit" }}>Ctrl + Win</kbd> to start dictating
            <div
              style={{
                position: "absolute",
                bottom: -5,
                left: "50%",
                transform: "translateX(-50%) rotate(45deg)",
                width: 10,
                height: 10,
                background: "var(--color-surface)",
                borderRight: "1px solid rgba(255,255,255,0.1)",
                borderBottom: "1px solid rgba(255,255,255,0.1)",
              }}
            />
          </div>
        )}

        {/* Tooltip — toggle hover (absolutely positioned above pill) */}
        {isContinuous && (
          <div
            style={{
              position: "absolute",
              bottom: "calc(100% + 10px)",
              left: "50%",
              transform: showToggleTooltip
                ? "translateX(-50%) translateY(0)"
                : "translateX(-50%) translateY(8px)",
              opacity: showToggleTooltip ? 1 : 0,
              pointerEvents: "none",
              transition:
                "opacity 200ms ease, transform 250ms cubic-bezier(0.2, 0.8, 0.2, 1)",
              transitionDelay: showToggleTooltip ? "80ms" : "0ms",
              whiteSpace: "nowrap",
              background: "var(--color-surface)",
              border: "1px solid rgba(255,255,255,0.1)",
              borderRadius: 100,
              padding: "10px 20px",
              fontSize: 13,
              color: "var(--color-text-secondary)",
              boxShadow: "0 4px 20px rgba(0,0,0,0.4)",
            }}
          >
            Press <kbd style={{ fontWeight: 700, color: "var(--color-text)", fontSize: 13, fontFamily: "inherit" }}>Ctrl + Win</kbd> again to stop dictating
            <div
              style={{
                position: "absolute",
                bottom: -5,
                left: "50%",
                transform: "translateX(-50%) rotate(45deg)",
                width: 10,
                height: 10,
                background: "var(--color-surface)",
                borderRight: "1px solid rgba(255,255,255,0.1)",
                borderBottom: "1px solid rgba(255,255,255,0.1)",
              }}
            />
          </div>
        )}

        {/* --- Content layers: always mounted, fade via opacity --- */}
        {/* overflow: hidden clips content within the shrinking pill boundary */}

        {/* Recording: Cancel + Waveform + Stop (both PTT and continuous) */}
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            overflow: "hidden",
            opacity: showRecording ? 1 : 0,
            transition: CONTENT_TRANSITION,
            pointerEvents: showRecording ? "auto" : "none",
          }}
        >
          <ContinuousControls showSideButtons={isContinuous} />
        </div>

        {/* Processing: animated dots */}
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            overflow: "hidden",
            opacity: showProcessing ? 1 : 0,
            transition: CONTENT_TRANSITION,
            pointerEvents: "none",
          }}
        >
          <StatusIndicator />
        </div>
      </div>
      </div>
    </div>
  );
}
