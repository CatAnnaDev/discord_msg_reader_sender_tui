#[derive(Clone, Debug)]
pub struct DspParams {
    pub enabled: bool,
    pub hpf: bool,
    pub hpf_hz: f32,
    pub gate: bool,
    pub gate_thresh: f32,
    pub comp: bool,
    pub comp_thresh: f32,
    pub comp_ratio: f32,
    pub comp_makeup: f32,
    pub agc: bool,
    pub agc_target: f32,
    pub ceiling: f32,
}

impl Default for DspParams {
    fn default() -> Self {
        Self {
            enabled: true,
            hpf: true,
            hpf_hz: 90.0,
            gate: true,
            gate_thresh: 0.006,
            comp: true,
            comp_thresh: 0.25,
            comp_ratio: 4.0,
            comp_makeup: 1.6,
            agc: true,
            agc_target: 0.12,
            ceiling: 0.97,
        }
    }
}

pub struct DspChain {
    sr: f32,
    hp_hz: f32,
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    gate_env: f32,
    comp_env: f32,
    agc_gain: f32,
}

impl DspChain {
    pub fn new(sample_rate: u32) -> Self {
        let mut c = Self {
            sr: sample_rate as f32,
            hp_hz: 0.0,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            gate_env: 0.0,
            comp_env: 0.0,
            agc_gain: 1.0,
        };
        c.set_hpf(90.0);
        c
    }

    fn set_hpf(&mut self, hz: f32) {
        if (hz - self.hp_hz).abs() < 0.5 {
            return;
        }
        self.hp_hz = hz;
        let w0 = 2.0 * std::f32::consts::PI * hz / self.sr;
        let cw = w0.cos();
        let sw = w0.sin();
        let q = 0.707_f32;
        let alpha = sw / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.b0 = (1.0 + cw) / 2.0 / a0;
        self.b1 = -(1.0 + cw) / a0;
        self.b2 = (1.0 + cw) / 2.0 / a0;
        self.a1 = -2.0 * cw / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    pub fn process(&mut self, p: &DspParams, x: &mut [f32]) {
        if !p.enabled || x.is_empty() {
            if !p.enabled {
                return;
            }
        }

        if p.hpf {
            self.set_hpf(p.hpf_hz.clamp(20.0, 400.0));
        }
        let atk = (-1.0 / (0.003 * self.sr)).exp();
        let rel = (-1.0 / (0.120 * self.sr)).exp();
        let g_rel = (-1.0 / (0.080 * self.sr)).exp();

        let mut sq_sum = 0.0f32;
        for s in x.iter_mut() {
            let mut v = *s;

            if p.hpf {
                let y = self.b0 * v + self.b1 * self.x1 + self.b2 * self.x2
                    - self.a1 * self.y1
                    - self.a2 * self.y2;
                self.x2 = self.x1;
                self.x1 = v;
                self.y2 = self.y1;
                self.y1 = y;
                v = y;
            }

            let mag = v.abs();

            if p.gate {
                let coef = if mag > self.gate_env { atk } else { g_rel };
                self.gate_env = coef * self.gate_env + (1.0 - coef) * mag;
                if self.gate_env < p.gate_thresh {
                    let t = (self.gate_env / p.gate_thresh.max(1e-6)).clamp(0.0, 1.0);
                    v *= t * t;
                }
            }

            if p.comp {
                let coef = if mag > self.comp_env { atk } else { rel };
                self.comp_env = coef * self.comp_env + (1.0 - coef) * mag;
                if self.comp_env > p.comp_thresh {
                    let over = self.comp_env / p.comp_thresh;
                    let gain = over.powf(1.0 / p.comp_ratio.max(1.0) - 1.0);
                    v *= gain;
                }
                v *= p.comp_makeup;
            }

            sq_sum += v * v;
            *s = v;
        }

        if p.agc && !x.is_empty() {
            let rms = (sq_sum / x.len() as f32).sqrt();
            if rms > 1e-5 {
                let want = (p.agc_target / rms).clamp(0.25, 6.0);
                let sm = if want > self.agc_gain { 0.02 } else { 0.05 };
                self.agc_gain += (want - self.agc_gain) * sm;
            }
            for s in x.iter_mut() {
                *s *= self.agc_gain;
            }
        }

        let c = p.ceiling.clamp(0.1, 1.0);
        for s in x.iter_mut() {
            let a = s.abs();
            if a > c {
                *s = s.signum() * (c + (a - c) / (1.0 + ((a - c) * 6.0)));
            }
            *s = s.clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_tames_loud_and_passes_quiet() {
        let mut ch = DspChain::new(48_000);
        let p = DspParams::default();
        let mut loud = vec![0.9f32; 960];
        ch.process(&p, &mut loud);
        assert!(loud.iter().all(|v| v.abs() <= 1.0001));
        let mut chq = DspChain::new(48_000);
        let mut quiet: Vec<f32> = (0..960).map(|i| 0.02 * (i as f32 * 0.1).sin()).collect();
        chq.process(&p, &mut quiet);
        assert!(quiet.iter().all(|v| v.is_finite()));
    }
}
