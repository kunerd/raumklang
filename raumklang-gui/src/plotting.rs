
pub struct ImpulseResponseChart {
    builder: WindowBuilder,
    base_chart: TimeseriesChart,
    cache: Cache,

    left_window_width: String,
    right_window_width: String,
}

impl ImpulseResponseChart {
    pub fn new(base_chart: TimeseriesChart) -> Self {
        let builder = WindowBuilder::new(Window::Tukey(0.25), Window::Tukey(0.25), 27562);
        let left_window_width = builder.get_left_side_width().to_string();
        let right_window_width = builder.get_right_side_width().to_string();

        Self {
            builder,
            base_chart,
            cache: Cache::new(),
            left_window_width,
            right_window_width,
        }
    }

    pub fn view(&self) -> Element<ImpulseResponseMessage> {
        let header: Element<_> = widget::row!(
            Text::new("Window:"),
            text_input("", &self.left_window_width)
                .on_input(ImpulseResponseMessage::LeftWidthChanged)
                .on_submit(ImpulseResponseMessage::LeftWidthSubmit),
            text_input("", &self.right_window_width)
                .on_input(ImpulseResponseMessage::RightWidthChanged)
                .on_submit(ImpulseResponseMessage::RightWidthSubmit),
        )
        .into();

        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(header)
                .push(ChartWidget::new(self).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .padding(10)
        .into()
    }

    pub fn update_msg(&mut self, msg: ImpulseResponseMessage) {
        match msg {
            ImpulseResponseMessage::LeftWidthChanged(s) => self.left_window_width = s,
            ImpulseResponseMessage::LeftWidthSubmit => {
                if let Ok(width) = self.left_window_width.parse() {
                    self.builder.set_left_side_width(width);
                    self.cache.clear();
                }
            }
            ImpulseResponseMessage::RightWidthChanged(s) => self.right_window_width = s,
            ImpulseResponseMessage::RightWidthSubmit => {
                if let Ok(width) = self.right_window_width.parse() {
                    self.builder.set_right_side_width(width);
                    self.cache.clear();
                }
            }
            ImpulseResponseMessage::TimeSeries(msg) => {
                self.cache.clear();
                self.base_chart.update_msg(msg);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImpulseResponseMessage {
    RightWidthChanged(String),
    RightWidthSubmit,
    LeftWidthChanged(String),
    LeftWidthSubmit,
    TimeSeries(TimeSeriesMessage),
}

impl Chart<ImpulseResponseMessage> for ImpulseResponseChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, builder: ChartBuilder<DB>) {
        let mut chart = self.base_chart.draw(builder);

        let window = self.builder.build();
        let max = window.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
        // FIXME: remove duplicate code with data processing
        let window = match &self.base_chart.amplitude_unit {
            Some(AmplitudeUnit::PercentFullScale) => {
                window.iter().map(|s| s / max * 100f32).collect()
            }
            Some(AmplitudeUnit::DezibelFullScale) => window
                .iter()
                .map(|s| {
                    let s = 20f32 * f32::log10(s / max);
                    // clip the signal
                    match (s.is_infinite(), s.is_sign_negative()) {
                        (true, true) => -100.0,
                        (true, false) => -100.0,
                        _ => s,
                    }
                })
                .collect(),
            None => window,
        };
        chart
            .draw_series(LineSeries::new(
                window.iter().enumerate().map(|(n, s)| (n as i64, *s)),
                &BLUE,
            ))
            .unwrap();
    }

    fn draw_chart<DB: DrawingBackend>(
        &self,
        state: &Self::State,
        root: DrawingArea<DB, plotters::coord::Shift>,
    ) {
        let builder = ChartBuilder::on(&root);
        self.build_chart(state, builder);
    }

    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<ImpulseResponseMessage>) {
        let (status, message) = self.base_chart.update(state, event, bounds, cursor);
        let msg = message.map(ImpulseResponseMessage::TimeSeries);

        (status, msg)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmplitudeUnit {
    #[default]
    PercentFullScale,
    DezibelFullScale,
}

impl AmplitudeUnit {
    const ALL: [AmplitudeUnit; 2] = [
        AmplitudeUnit::PercentFullScale,
        AmplitudeUnit::DezibelFullScale,
    ];
}

impl std::fmt::Display for AmplitudeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AmplitudeUnit::PercentFullScale => "%FS",
                AmplitudeUnit::DezibelFullScale => "dbFS",
            }
        )
    }
}

#[derive(Debug, Clone)]
pub enum TimeSeriesMessage {
    MouseEvent(mouse::Event, iced::Point),
    EventOccured(Event),
    AmplitudeUnitChanged(AmplitudeUnit),
}

pub struct TimeseriesChart {
    shift: bool,
    cache: Cache,
    name: String,
    data: Vec<(i64, f32)>,
    processed_data: Vec<(i64, f32)>,
    min: f32,
    max: f32,
    viewport: Range<i64>,
    spec: RefCell<Option<Cartesian2d<RangedCoordi64, RangedCoordf32>>>,
    amplitude_unit: Option<AmplitudeUnit>,
}

impl TimeseriesChart {
    fn new(
        name: String,
        data: impl Iterator<Item = (i64, f32)>,
        amplitude_unit: Option<AmplitudeUnit>,
    ) -> Self {
        let data: Vec<_> = data.collect();
        let viewport = 0..data.len() as i64;
        let mut chart = Self {
            name,
            data,
            min: f32::NEG_INFINITY,
            max: f32::INFINITY,
            processed_data: vec![],
            cache: Cache::new(),
            viewport,
            spec: RefCell::new(None),
            amplitude_unit,
            shift: false,
        };
        chart.process_data();
        chart
    }

    fn view(&self) -> Element<TimeSeriesMessage> {
        let header = widget::row!(
            Text::new(&self.name),
            widget::pick_list(
                &AmplitudeUnit::ALL[..],
                self.amplitude_unit,
                TimeSeriesMessage::AmplitudeUnitChanged
            ),
        );

        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(header)
                .push(ChartWidget::new(self).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    fn update_msg(&mut self, msg: TimeSeriesMessage) {
        match msg {
            TimeSeriesMessage::MouseEvent(evt, point) => match evt {
                //mouse::Event::CursorEntered => todo!(),
                //mouse::Event::CursorLeft => todo!(),
                //mouse::Event::CursorMoved { position } => todo!(),
                //mouse::Event::ButtonPressed(_) => todo!(),
                //mouse::Event::ButtonReleased(_) => todo!(),
                mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { y, .. },
                } => {
                    match self.shift {
                        true => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.scroll_right();
                            } else {
                                self.scroll_left();
                            }
                        }
                        false => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.zoom_in(point);
                            } else {
                                self.zoom_out(point);
                            }
                        }
                    }
                }
                _ => {}
            },
            TimeSeriesMessage::EventOccured(event) => {
                if let Event::Keyboard(event) = event {
                    match event {
                        iced::keyboard::Event::KeyPressed {
                            key_code,
                            modifiers: _,
                        } => match key_code {
                            iced::keyboard::KeyCode::LShift => self.shift = true,
                            iced::keyboard::KeyCode::RShift => self.shift = true,
                            _ => {}
                        },
                        iced::keyboard::Event::KeyReleased {
                            key_code,
                            modifiers: _,
                        } => match key_code {
                            iced::keyboard::KeyCode::LShift => self.shift = false,
                            iced::keyboard::KeyCode::RShift => self.shift = false,
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
            TimeSeriesMessage::AmplitudeUnitChanged(u) => self.set_amplitude_unit(u),
        }
    }

    fn zoom_in(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 0.8;
                const LOWER_BOUND: i64 = 256;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len < LOWER_BOUND {
                    new_len = LOWER_BOUND;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.viewport = new_start..new_end;

                self.cache.clear();
            }
        }
    }

    fn zoom_out(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 1.2;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len >= self.data.len() as i64 {
                    new_len = self.data.len() as i64;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.viewport = new_start..new_end;

                self.cache.clear();
            }
        }
    }

    fn scroll_right(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let mut new_end = old_viewport.end.saturating_add(offset);
        if new_end > self.data.len() as i64 {
            new_end = self.data.len() as i64;
        }

        let new_start = new_end - length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn scroll_left(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let new_start = old_viewport.start.saturating_sub(offset);
        let new_end = new_start + length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn process_data(&mut self) {
        //let max = self.data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
        let max = self
            .data
            .iter()
            .map(|s| s.1.powi(2).sqrt())
            .fold(f32::NEG_INFINITY, |a, b| a.max(b));

        // FIXME: precompute on amplitude change
        self.processed_data = match &self.amplitude_unit {
            Some(AmplitudeUnit::PercentFullScale) => self
                .data
                .iter()
                .map(|(n, s)| (*n, *s / max * 100f32))
                .collect(),
            Some(AmplitudeUnit::DezibelFullScale) => self
                .data
                .iter()
                .map(|(n, s)| (*n, 20f32 * f32::log10(s.abs() / max)))
                .collect(),
            None => self.data.clone(),
        };

        self.min = self
            .processed_data
            .iter()
            .fold(f32::INFINITY, |a, b| a.min(b.1));
        self.max = self
            .processed_data
            .iter()
            .fold(f32::NEG_INFINITY, |a, b| a.max(b.1));

        self.cache.clear();
    }

    fn set_amplitude_unit(&mut self, u: AmplitudeUnit) {
        self.amplitude_unit = Some(u);

        self.process_data();
    }

    fn draw<'a, DB: DrawingBackend>(
        &'a self,
        mut builder: ChartBuilder<'a, 'a, DB>,
    ) -> ChartContext<DB, Cartesian2d<RangedCoordi64, RangedCoordf32>> {
        use plotters::prelude::*;

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(self.viewport.clone(), self.min..self.max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(self.processed_data.iter().cloned(), &RED))
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();

        *self.spec.borrow_mut() = Some(chart.as_coord_spec().clone());

        chart
    }
}

impl Chart<TimeSeriesMessage> for TimeseriesChart {
    type State = ();
    // fn update(
    //     &mut self,
    //     event: Event,
    //     bounds: Rectangle,
    //     cursor: Cursor,
    // ) -> (event::Status, Option<Message>) {
    //     self.cache.clear();
    //     (event::Status::Ignored, None)
    // }

    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut chart: ChartBuilder<DB>) {
        self.draw(chart);
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<TimeSeriesMessage>) {
        if let mouse::Cursor::Available(point) = cursor {
            match event {
                canvas::Event::Mouse(evt) if bounds.contains(point) => {
                    let p_origin = bounds.position();
                    let p = point - p_origin;
                    return (
                        event::Status::Captured,
                        Some(TimeSeriesMessage::MouseEvent(
                            evt,
                            iced::Point::new(p.x, p.y),
                        )),
                    );
                }
                _ => {}
            }
        }
        (event::Status::Ignored, None)
    }
}

enum TimeSeriesRange {
    Samples(RangedCoordusize),
    Time(usize, RangedCoordusize),
}

impl ValueFormatter<usize> for TimeSeriesRange {
    fn format_ext(&self, value: &usize) -> String {
        match self {
            TimeSeriesRange::Samples(_) => format!("{}", value),
            TimeSeriesRange::Time(sample_rate, _) => {
                format!("{}", *value as f32 / *sample_rate as f32)
            }
        }
    }
}

impl Ranged for TimeSeriesRange {
    type FormatOption = NoDefaultFormatting;

    type ValueType = usize;

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        match self {
            TimeSeriesRange::Samples(ranged) => ranged.map(value, limit),
            TimeSeriesRange::Time(_sample_rate, ranged) => ranged.map(value, limit),
        }
    }

    fn key_points<Hint: plotters::coord::ranged1d::KeyPointHint>(
        &self,
        hint: Hint,
    ) -> Vec<Self::ValueType> {
        match self {
            TimeSeriesRange::Samples(ranged) => ranged.key_points(hint),
            TimeSeriesRange::Time(_, ranged) => ranged.key_points(hint),
        }
    }

    fn range(&self) -> Range<Self::ValueType> {
        match self {
            TimeSeriesRange::Samples(ranged) => ranged.range(),
            TimeSeriesRange::Time(_, ranged) => ranged.range(),
        }
    }

    fn axis_pixel_range(&self, limit: (i32, i32)) -> Range<i32> {
        if limit.0 < limit.1 {
            limit.0..limit.1
        } else {
            limit.1..limit.0
        }
    }
}

pub struct TimeseriesChartNew {
    cache: Cache,
    data: Vec<f32>,
    time_unit: TimeSeriesUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TimeSeriesUnit {
    Samples,
    Time,
}

impl TimeSeriesUnit {
    const ALL: [Self; 2] = [TimeSeriesUnit::Samples, TimeSeriesUnit::Time];
}

impl std::fmt::Display for TimeSeriesUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TimeSeriesUnit::Samples => "Samples",
                TimeSeriesUnit::Time => "Time",
            }
        )
    }
}

#[derive(Debug, Clone)]
enum TimeSeriesMessageNew {
    TimeUnitChanged(TimeSeriesUnit),
}

impl TimeseriesChartNew {
    fn new(data: impl Iterator<Item = f32>, time_unit: TimeSeriesUnit) -> Self {
        let data = data.collect();
        Self {
            data,
            time_unit,
            cache: Cache::new(),
        }
    }

    fn view(&self) -> Element<TimeSeriesMessageNew> {
        let header = widget::row!(widget::pick_list(
            &TimeSeriesUnit::ALL[..],
            Some(self.time_unit.clone()),
            TimeSeriesMessageNew::TimeUnitChanged
        ));
        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(header)
                .push(ChartWidget::new(self).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    fn update_msg(&mut self, msg: TimeSeriesMessageNew) {
        match msg {
            TimeSeriesMessageNew::TimeUnitChanged(u) => {
                self.time_unit = u;
                self.cache.clear();
            }
        }
    }
}

impl Chart<TimeSeriesMessageNew> for TimeseriesChartNew {
    type State = ();

    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use plotters::prelude::*;

        let x_range = match self.time_unit {
            TimeSeriesUnit::Samples => TimeSeriesRange::Samples((0..self.data.len()).into()),
            TimeSeriesUnit::Time => TimeSeriesRange::Time(44100, (0..self.data.len()).into()),
        };

        let min = self.data.iter().fold(f32::INFINITY, |a, b| a.min(*b));
        let max = self.data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(x_range, min..max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(self.data.iter().cloned().enumerate(), &RED))
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();
    }
}
