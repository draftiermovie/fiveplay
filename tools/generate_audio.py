"""Generate original, deterministic placeholder sound assets; no external downloads."""
import math, random, struct, wave
from pathlib import Path
root = Path(__file__).resolve().parents[1] / 'assets' / 'audio'
root.mkdir(parents=True, exist_ok=True)
def save(name, samples):
    with wave.open(str(root / name), 'wb') as f:
        f.setparams((1, 2, 22050, 0, 'NONE', 'not compressed'))
        f.writeframes(b''.join(struct.pack('<h', int(max(-1, min(1, s)) * 24000)) for s in samples))
for index, duration in enumerate([.19, .12, .45, .35, .16]):
    rng = random.Random(index)
    save(f'shot{index}.wav', [(rng.uniform(-1,1)*.75 + math.sin(i/22050*math.tau*(90+index*17))*.25) * math.exp(-i/22050*24) for i in range(int(22050*duration))])
save('menu.wav', [sum(math.sin(math.tau*f*i/22050)*.045 for f in [55,82.5,110,164.999]) * min(1,i/22050) * min(1,(22050*8-i)/22050) for i in range(22050*8)])
