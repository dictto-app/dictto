import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../stores/appStore";

interface WaveformPayload {
  amplitudes: number[];
}

export function useWaveform() {
  const { waveformData, setWaveformData } = useAppStore();

  useEffect(() => {
    const unlisten = listen<WaveformPayload>("waveform-data", (event) => {
      setWaveformData(event.payload.amplitudes);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setWaveformData]);

  return { waveformData };
}
