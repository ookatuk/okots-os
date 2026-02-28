use uart_16550::SerialPort;
use spin::Mutex;

pub static SERIAL1: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3f8) });

pub fn init_serial() {
    SERIAL1.lock().init();
}