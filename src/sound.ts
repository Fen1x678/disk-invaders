// 8-bit sound effects synthesized on the fly with the Web Audio API. No audio files needed.

let ctx: AudioContext | null = null;
let enabled = true;

export function setSoundEnabled(on: boolean): void {
  enabled = on;
}

function audio(): AudioContext | null {
  if (!enabled || typeof AudioContext === "undefined") return null;
  ctx ??= new AudioContext();
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

interface ToneOptions {
  type?: OscillatorType;
  volume?: number;
  slideTo?: number;
  delay?: number;
}

function tone(freq: number, duration: number, { type = "square", volume = 0.05, slideTo, delay = 0 }: ToneOptions = {}): void {
  const ac = audio();
  if (!ac) return;
  const start = ac.currentTime + delay;
  const osc = ac.createOscillator();
  const gain = ac.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, start);
  if (slideTo) osc.frequency.exponentialRampToValueAtTime(slideTo, start + duration);
  gain.gain.setValueAtTime(volume, start);
  gain.gain.exponentialRampToValueAtTime(0.0001, start + duration);
  osc.connect(gain).connect(ac.destination);
  osc.start(start);
  osc.stop(start + duration + 0.02);
}

function noise(duration: number, volume = 0.18): void {
  const ac = audio();
  if (!ac) return;
  const buffer = ac.createBuffer(1, Math.floor(ac.sampleRate * duration), ac.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < data.length; i++) data[i] = Math.random() * 2 - 1;
  const source = ac.createBufferSource();
  source.buffer = buffer;
  const filter = ac.createBiquadFilter();
  filter.type = "lowpass";
  filter.frequency.setValueAtTime(3000, ac.currentTime);
  filter.frequency.exponentialRampToValueAtTime(80, ac.currentTime + duration);
  const gain = ac.createGain();
  gain.gain.setValueAtTime(volume, ac.currentTime);
  gain.gain.exponentialRampToValueAtTime(0.0001, ac.currentTime + duration);
  source.connect(filter).connect(gain).connect(ac.destination);
  source.start();
}

const arpeggio = (notes: number[], step: number, length: number) =>
  notes.forEach((freq, i) => tone(freq, length, { delay: i * step }));

export const sfx = {
  blip: () => tone(880, 0.05),
  select: () => tone(660, 0.07, { slideTo: 990 }),
  deselect: () => tone(520, 0.07, { slideTo: 330 }),
  start: () => arpeggio([523, 659, 784, 1047], 0.08, 0.09),
  coin: () => {
    tone(988, 0.08);
    tone(1319, 0.35, { delay: 0.08 });
  },
  laser: () => tone(1500, 0.22, { type: "sawtooth", slideTo: 110, volume: 0.04 }),
  explode: () => noise(0.6),
  error: () => tone(150, 0.25, { type: "sawtooth", volume: 0.04 }),
  victory: () => arpeggio([523, 659, 784, 1047, 784, 1047], 0.1, 0.12),
};
