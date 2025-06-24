use ringbuf::Rb;

pub struct MeterProd(ringbuf::HeapProducer<f32>);

impl MeterProd {
    // TODO: Result for dropped consumer
    pub fn push_iter(&mut self, iter: &mut impl Iterator<Item = f32>) -> usize {
        self.0.push_iter(iter)
    }
}

pub struct MeterConsumer {
    buf: ringbuf::HeapConsumer<f32>,
    meter: Meter,
}

impl MeterConsumer {
    pub fn update(&mut self) -> bool {
        self.meter.update_from_iter(self.buf.pop_iter())
    }
}

pub struct Meter {
    peak: f32,
    square_sum: f32,
    window_size: usize,
    buf: ringbuf::HeapRb<f32>,
}

impl Meter {
    pub fn new(window_size: usize) -> Self {
        let buf = ringbuf::HeapRb::<_>::new(window_size);
        Self {
            square_sum: 0.0,
            peak: f32::NEG_INFINITY,
            window_size,
            buf,
        }
    }

    pub fn update_from_iter<I>(&mut self, iter: I) -> bool
    where
        I: IntoIterator<Item = f32>,
    {
        let mut new_peak = false;

        for s in iter {
            new_peak = new_peak || self.update(s);
        }

        new_peak
    }

    pub fn update(&mut self, sample: f32) -> bool {
        let sample_squared = sample * sample;
        self.square_sum += sample_squared;

        let mut new_peak = false;
        if self.peak < sample {
            self.peak = sample;
            new_peak = true;
        }

        let removed = self.buf.push_overwrite(sample_squared);
        if let Some(r) = removed {
            self.square_sum -= r;
        }

        new_peak
    }

    pub fn rms(&self) -> f32 {
        (self.square_sum / (self.window_size as f32)).sqrt()
    }

    pub fn peak(&self) -> f32 {
        self.peak
    }

    pub fn reset_peak(&mut self) {
        self.peak = f32::NEG_INFINITY;
        for s in self.buf.iter() {
            self.peak = self.peak.max(*s)
        }
    }
}
