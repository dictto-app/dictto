import { useWaveform } from "../../hooks/useWaveform";
import { useRef } from "react";

interface Props {
  barCount?: number;
}

function resampleBars(data: number[], targetCount: number): number[] {
  if (data.length === 0) return Array(targetCount).fill(0.05);
  if (data.length === targetCount) return data;

  const result: number[] = [];
  for (let i = 0; i < targetCount; i++) {
    const srcIndex = (i / (targetCount - 1)) * (data.length - 1);
    const low = Math.floor(srcIndex);
    const high = Math.min(low + 1, data.length - 1);
    const frac = srcIndex - low;
    result.push(data[low] * (1 - frac) + data[high] * frac);
  }
  return result;
}

function generateIdleBars(count: number): number[] {
  return Array.from({ length: count }, () => 0.05 + Math.random() * 0.05);
}

export function Waveform({ barCount = 20 }: Props) {
  const { waveformData } = useWaveform();
  const idleBarsRef = useRef<number[]>(generateIdleBars(barCount));

  const bars =
    waveformData.length > 0
      ? resampleBars(waveformData, barCount)
      : idleBarsRef.current;

  const containerHeight = 24;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 2.5,
        height: containerHeight,
      }}
    >
      {bars.map((amplitude, i) => {
        const barHeight = Math.max(2, amplitude * containerHeight * 0.85);
        return (
          <div
            key={i}
            style={{
              width: 3,
              height: barHeight,
              borderRadius: 2,
              background: "rgba(255, 255, 255, 0.85)",
              transition: "height 50ms",
              flexShrink: 0,
            }}
          />
        );
      })}
    </div>
  );
}
