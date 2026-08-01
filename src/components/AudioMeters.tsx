export interface MeterData {
  rms: number;
  peak: number;
}

export interface AudioMetersProps {
  system?: MeterData;
  microphone?: MeterData;
}

/** 音量表：系统音频与麦克风两路独立显示。 */
export function AudioMeters({ system, microphone }: AudioMetersProps) {
  return (
    <div className="audio-meters" data-testid="audio-meters">
      <div className="audio-meters__row">
        <span className="audio-meters__label">系统音频</span>
        <div
          className="audio-meters__track"
          role="meter"
          aria-label="系统音频音量"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round((system?.rms ?? 0) * 100)}
        >
          <div
            className="audio-meters__fill"
            data-active={Boolean(system && system.rms > 0.001)}
            style={{ width: `${Math.min(100, Math.max(2, (system?.peak ?? 0) * 100))}%` }}
          />
        </div>
      </div>
      <div className="audio-meters__row">
        <span className="audio-meters__label">麦克风</span>
        <div
          className="audio-meters__track"
          role="meter"
          aria-label="麦克风音量"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round((microphone?.rms ?? 0) * 100)}
        >
          <div
            className="audio-meters__fill"
            data-active={Boolean(microphone && microphone.rms > 0.001)}
            style={{ width: `${Math.min(100, Math.max(2, (microphone?.peak ?? 0) * 100))}%` }}
          />
        </div>
      </div>
    </div>
  );
}
