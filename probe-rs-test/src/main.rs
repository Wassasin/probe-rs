use std::time::Duration;

use probe_rs::{
    CoreInterface, MemoryInterface, Permissions,
    architecture::arm::{
        FullyQualifiedApAddress,
        ap::{ApRegister, CSW},
    },
    probe::list::Lister,
    rtt::{Rtt, ScanRegion},
};
use tracing_subscriber::FmtSubscriber;

pub fn get_rtt_symbol_from_bytes(buffer: &[u8]) -> Result<u64, ()> {
    match goblin::elf::Elf::parse(buffer) {
        Ok(binary) => {
            for sym in &binary.syms {
                if binary.strtab.get_at(sym.st_name) == Some("_SEGGER_RTT") {
                    return Ok(sym.st_value);
                }
            }
            Err(())
        }
        Err(_err) => Err(()),
    }
}

pub fn get_main_symbol_from_bytes(buffer: &[u8]) -> Result<u64, ()> {
    match goblin::elf::Elf::parse(buffer) {
        Ok(binary) => {
            for sym in &binary.syms {
                if binary.strtab.get_at(sym.st_name) == Some("main") {
                    return Ok(sym.st_value);
                }
            }
            Err(())
        }
        Err(_err) => Err(()),
    }
}

fn main() -> Result<(), anyhow::Error> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let elf = std::fs::read(
        "/home/wouter/sarif/embassy-imxrt/examples/rt685s-evk/target/thumbv8m.main-none-eabihf/release/hello-world",
    )?;

    let scan_regions = ScanRegion::Exact(get_rtt_symbol_from_bytes(&elf).unwrap());
    let main = get_main_symbol_from_bytes(&elf).unwrap();

    let lister = Lister::new();

    let probes = async_io::block_on(lister.list_all());

    // Use the first probe found.
    let probe = probes[0].open()?;

    let mut session = probe.attach("MIMXRT685SFVKB", Permissions::default())?;

    // Select a core.
    let mut core = session.core(0)?;

    {
        tracing::info!("Fetch NXP CSW");
        let csw = core.read_word_32(0x4008_B000)?;
        tracing::info!("NXP CSW: {:x}", csw);
    }

    // tracing::info!("Starting reset and halt");
    core.reset()?;

    drop(core);

    {
        let interface = session.get_arm_interface()?;
        let ap = FullyQualifiedApAddress::v1_with_default_dp(0);
        let csw = interface.read_raw_ap_register(&ap, 0x00)?;
        let csw: CSW = csw.try_into()?;
        tracing::info!("CSW: {:?}", csw);
    }

    let mut core = session.core(0)?;

    // core.reset_and_halt(Duration::from_secs(1))?;
    std::thread::sleep(Duration::from_millis(100));

    tracing::info!("Calling halt");
    core.halt(Duration::from_millis(1000))?;

    // interface.read_raw_ap_register(ap, 0x00)?
    tracing::info!("Starting run");

    // core.set_hw_breakpoint(main)?;
    core.run()?;

    // std::thread::sleep(Duration::from_millis(100));

    tracing::info!("Running");

    drop(core);

    {
        let interface = session.get_arm_interface()?;
        let ap = FullyQualifiedApAddress::v1_with_default_dp(0);
        let csw = interface.read_raw_ap_register(&ap, 0x00)?;
        let csw: CSW = csw.try_into()?;
        tracing::info!("CSW: {:?}", csw);
    }

    let mut core = session.core(0)?;

    {
        tracing::info!("Fetch NXP CSW");
        let csw = core.read_word_32(0x4008_B000)?;
        tracing::info!("NXP CSW: {:x}", csw);
    }

    std::thread::sleep(Duration::from_millis(1000));

    let mut rtt = Rtt::attach_region(&mut core, &scan_regions)?;

    tracing::info!("{:?}", rtt.up_channels());

    if let Some(input) = rtt.up_channel(0) {
        loop {
            let mut buf = [0u8; 1024];
            let count = input.read(&mut core, &mut buf[..])?;

            if count > 0 {
                println!("Read data: {:?}", &buf[..count]);
            }
        }
    }

    Ok(())
}
