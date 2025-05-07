use ppa6::Printer;

fn main() {
    let mut printer = Printer::find().unwrap();
    printer.reset().expect("cannot reset printer");
    println!("IP:            {}", printer.get_ip().unwrap());
    println!("Name:          {}", printer.get_name().unwrap());
    println!("Serial:        {}", printer.get_serial().unwrap());
    println!("Firmware Ver.: {}", printer.get_firmware_ver().unwrap());
    println!("Hardware Ver.: {}", printer.get_hardware_ver().unwrap());
    println!("Battery Level: {}", printer.get_battery().unwrap());
    println!("MAC address:   {}", printer.get_mac().unwrap());
}
