import { create } from "zustand";

export type RecordingState = "idle" | "recording" | "processing";

export type RecordingMode = "hold" | "continuous" | null;

interface AppStore {
  recordingState: RecordingState;
  recordingMode: RecordingMode;
  waveformData: number[];
  setRecordingState: (state: RecordingState, mode?: RecordingMode) => void;
  setWaveformData: (data: number[]) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  recordingState: "idle",
  recordingMode: null,
  waveformData: [],
  setRecordingState: (recordingState, recordingMode = null) =>
    set({ recordingState, recordingMode }),
  setWaveformData: (waveformData) => set({ waveformData }),
}));
