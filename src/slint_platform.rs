use core::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const DISPLAY_WIDTH: usize = 800;
const DISPLAY_HEIGHT: usize = 480;

type I2C = esp_idf_svc::hal::i2c::I2cDriver<'static>;
type Gt911 = gt911::Gt911Blocking<I2C>;

struct EspPlatform {
    panel_handle: esp_idf_svc::sys::esp_lcd_panel_handle_t,
    touch: Gt911,
    i2c: RefCell<I2C>,
    window: Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
    timer: esp_idf_svc::timer::EspTimerService<esp_idf_svc::timer::Task>,
    queue: Arc<Mutex<Vec<Event>>>,
}

impl EspPlatform {
    pub fn new(mut i2c: I2C) -> std::boxed::Box<Self> {
        use esp_idf_svc::hal::sys::*;

        // Initialize LCD panel and touch
        let mut panel_handle: esp_lcd_panel_handle_t = std::ptr::null_mut();
        let panel_config = esp_lcd_rgb_panel_config_t {
            clk_src: soc_module_clk_t_SOC_MOD_CLK_PLL_F160M, //LCD_CLK_SRC_DEFAULT,
            timings: esp_lcd_rgb_timing_t {
                pclk_hz: 16 * 1000 * 1000,
                h_res: DISPLAY_WIDTH as u32,
                v_res: DISPLAY_HEIGHT as u32,
                hsync_pulse_width: 4,
                hsync_back_porch: 8,
                hsync_front_porch: 8,
                vsync_pulse_width: 4,
                vsync_back_porch: 8,
                vsync_front_porch: 8,
                flags: {
                    let mut flags = esp_lcd_rgb_timing_t__bindgen_ty_1::default();
                    flags.set_pclk_active_neg(1);
                    flags
                },
            },
            data_width: 16,
            bits_per_pixel: 16,
            num_fbs: 2,
            bounce_buffer_size_px: DISPLAY_WIDTH * 10,
            sram_trans_align: 4,
            __bindgen_anon_1: esp_lcd_rgb_panel_config_t__bindgen_ty_1 { dma_burst_size: 64 },
            hsync_gpio_num: 46,
            vsync_gpio_num: 3,
            de_gpio_num: 5,
            pclk_gpio_num: 7,
            disp_gpio_num: -1,
            data_gpio_nums: [14, 38, 18, 17, 10, 39, 0, 45, 48, 47, 21, 1, 2, 42, 41, 40],
            flags: {
                let mut flags = esp_lcd_rgb_panel_config_t__bindgen_ty_2::default();
                flags.set_fb_in_psram(1);
                flags
            },
        };
        unsafe {
            assert_eq!(
                esp_lcd_new_rgb_panel(&panel_config, &mut panel_handle),
                ESP_OK
            );
            assert_eq!(esp_lcd_panel_init(panel_handle), ESP_OK);
            assert_eq!(
                esp_lcd_rgb_panel_register_event_callbacks(
                    panel_handle,
                    &esp_lcd_rgb_panel_event_callbacks_t {
                        on_vsync: Some(vsync_callback),
                        ..Default::default()
                    },
                    core::ptr::null_mut()
                ),
                ESP_OK
            );
        }

        // Setup the touch
        let touch = Gt911::default();
        touch.init(&mut i2c).unwrap();

        // Setup the window
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::SwappedBuffers,
        );
        window.set_size(slint::PhysicalSize::new(
            DISPLAY_WIDTH as u32,
            DISPLAY_HEIGHT as u32,
        ));

        std::boxed::Box::new(Self {
            panel_handle,
            touch,
            i2c: i2c.into(),
            window,
            timer: esp_idf_svc::timer::EspTimerService::new().unwrap(),
            queue: Default::default(),
        })
    }
}

impl slint::platform::Platform for EspPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        // Since on MCUs, there can be only one window, just return a clone of self.window.
        // We'll also use the same window in the event loop.
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        self.timer.now()
    }
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        use esp_idf_svc::hal::sys::*;

        unsafe {
            // Initialize the LCD panel
            if esp_lcd_panel_init(self.panel_handle) != ESP_OK {
                log::error!("Failed to initialize LCD panel");
                return Err(slint::PlatformError::Other(
                    "Failed to initialize LCD panel".into(),
                ));
            }

            // Turn on the display
            esp_lcd_panel_disp_on_off(self.panel_handle, true);

            // Calling this function rotates the display by 180 degrees
            esp_lcd_panel_mirror(self.panel_handle, true, true);
        }

        // Create a buffer to draw the scene
        use slint::platform::software_renderer::Rgb565Pixel;

        let (mut buffer1, mut buffer2) = unsafe {
            let (mut b1, mut b2) = (std::ptr::null_mut(), std::ptr::null_mut());
            esp_lcd_rgb_panel_get_frame_buffer(self.panel_handle, 2, &mut b1, &mut b2);
            (
                core::slice::from_raw_parts_mut(
                    b1 as *mut Rgb565Pixel,
                    DISPLAY_WIDTH * DISPLAY_HEIGHT,
                ),
                core::slice::from_raw_parts_mut(
                    b2 as *mut Rgb565Pixel,
                    DISPLAY_WIDTH * DISPLAY_HEIGHT,
                ),
            )
        };

        let mut last_position = slint::LogicalPosition::default();
        let mut touch_down = false;

        loop {
            slint::platform::update_timers_and_animations();

            let queue = std::mem::take(&mut *self.queue.lock().unwrap());
            for event in queue {
                match event {
                    Event::Invoke(event) => event(),
                    Event::Quit => break,
                }
            }

            match self.touch.get_touch(&mut self.i2c.borrow_mut()) {
                Ok(Some(point)) => {
                    last_position = slint::PhysicalPosition::new(point.x as _, point.y as _)
                        .to_logical(self.window.scale_factor());
                    if !touch_down {
                        self.window
                            .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                                position: last_position,
                                button: slint::platform::PointerEventButton::Left,
                            });
                    }
                    self.window
                        .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                            position: last_position,
                        });
                    touch_down = true;
                }
                Ok(None) => {
                    if touch_down {
                        self.window
                            .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                                position: last_position,
                                button: slint::platform::PointerEventButton::Left,
                            });
                        self.window
                            .dispatch_event(slint::platform::WindowEvent::PointerExited);
                    }
                    touch_down = false;
                }
                Err(gt911::Error::NotReady) => {
                    //skip
                }
                Err(err) => {
                    log::error!("Error reading the touch screen: {:?}", err);
                }
            }

            // Draw the scene if something needs to be drawn.
            self.window.draw_if_needed(|renderer| {
                while !VSYNC.load(core::sync::atomic::Ordering::SeqCst) {
                    esp_idf_svc::hal::task::do_yield();
                }
                renderer.render(buffer1, DISPLAY_WIDTH);
                unsafe {
                    esp_lcd_panel_draw_bitmap(
                        self.panel_handle,
                        0,
                        0,
                        DISPLAY_WIDTH as i32,
                        DISPLAY_HEIGHT as i32,
                        buffer1.as_ptr().cast(),
                    )
                };
                VSYNC.store(false, core::sync::atomic::Ordering::SeqCst);

                core::mem::swap(&mut buffer1, &mut buffer2);
            });

            // Try to put the MCU to sleep
            if !self.window.has_active_animations() {
                continue;
            }

            // FIXME
            esp_idf_svc::hal::task::do_yield();
        }
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        log::debug!("{}", arguments);
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(EspEventLoopProxy {
            queue: self.queue.clone(),
        }))
    }
}

enum Event {
    Quit,
    Invoke(Box<dyn FnOnce() + Send>),
}
struct EspEventLoopProxy {
    queue: Arc<Mutex<Vec<Event>>>,
}
impl slint::platform::EventLoopProxy for EspEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        self.queue.lock().unwrap().push(Event::Quit);
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        self.queue.lock().unwrap().push(Event::Invoke(event));
        Ok(())
    }
}

pub fn init(i2c: I2C) {
    slint::platform::set_platform(EspPlatform::new(i2c)).unwrap();
}

static VSYNC: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

extern "C" fn vsync_callback(
    _panel: esp_idf_svc::hal::sys::esp_lcd_panel_handle_t,
    _edata: *const esp_idf_svc::hal::sys::esp_lcd_rgb_panel_event_data_t,
    _user_ctx: *mut core::ffi::c_void,
) -> bool {
    VSYNC.store(true, core::sync::atomic::Ordering::SeqCst);
    false
}