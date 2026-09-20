#![allow(clippy::empty_loop)]
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use panic_semihosting as _;

use stuf_env::clock::FixedClock;
use stuf_env::transport::Transport;
use stuf_tuf::verify::no_heap::TrustAnchor;

static ROOT_BYTES: &[u8] = include_bytes!("../../toaster/factory/root.json");

const CURRENT_VERSION: &str = "1.0.0";
const NEW_VERSION: &str = "1.1.0";
const TARGET_FIRMWARE: &str = "toaster-firmware-1.1.0.bin";

const META_BUF: usize = 4096;
const FIRMWARE_BUF: usize = 2048;
const PATH_BUF: usize = 128;

static mut TS_BUF: [u8; META_BUF] = [0; META_BUF];
static mut SNAP_BUF: [u8; META_BUF] = [0; META_BUF];
static mut TARGETS_BUF: [u8; META_BUF] = [0; META_BUF];
static mut FIRMWARE_BUF_BYTES: [u8; FIRMWARE_BUF] = [0; FIRMWARE_BUF];

unsafe fn static_buf<const N: usize>(ptr: *mut [u8; N]) -> &'static mut [u8] {
    core::slice::from_raw_parts_mut(ptr as *mut u8, N)
}

mod semi {
    use cortex_m_semihosting::nr;

    pub fn open(path: &[u8]) -> Option<usize> {
        let args = [path.as_ptr() as usize, 0usize, path.len() - 1];
        let fd = unsafe { cortex_m_semihosting::syscall(nr::OPEN, &args) };
        if fd == usize::MAX {
            None
        } else {
            Some(fd)
        }
    }

    pub fn flen(fd: usize) -> Option<usize> {
        let len = unsafe { cortex_m_semihosting::syscall(nr::FLEN, &fd) };
        if len == usize::MAX {
            None
        } else {
            Some(len)
        }
    }

    pub fn read(fd: usize, buf: &mut [u8]) -> bool {
        let args = [fd, buf.as_mut_ptr() as usize, buf.len()];
        unsafe { cortex_m_semihosting::syscall(nr::READ, &args) == 0 }
    }

    pub fn close(fd: usize) {
        unsafe { cortex_m_semihosting::syscall(nr::CLOSE, &fd) };
    }
}

fn build_path(filename: &str, buf: &mut [u8; PATH_BUF]) -> usize {
    let prefix = b"stuf-examples/.generated/publisher-repo/";
    let fname = filename.as_bytes();
    let total = prefix.len() + fname.len() + 1;

    if total > PATH_BUF {
        return 0;
    }

    buf[..prefix.len()].copy_from_slice(prefix);
    buf[prefix.len()..prefix.len() + fname.len()].copy_from_slice(fname);
    buf[prefix.len() + fname.len()] = 0;

    total
}

fn fetch_to_buf<'a>(filename: &str, buf: &'a mut [u8]) -> Option<&'a [u8]> {
    let mut path = [0u8; PATH_BUF];
    let path_len = build_path(filename, &mut path);
    if path_len == 0 {
        return None;
    }

    let fd = semi::open(&path[..path_len])?;
    let len = semi::flen(fd)?;
    if len > buf.len() {
        semi::close(fd);
        return None;
    }

    let ok = semi::read(fd, &mut buf[..len]);
    semi::close(fd);

    if ok {
        Some(&buf[..len])
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct NoTransportError;

#[derive(Clone, Copy, Debug)]
struct NoTransport;

impl Transport for NoTransport {
    type Buffer = &'static [u8];
    type Error = NoTransportError;

    fn fetch(&self, _id: &str) -> Result<Self::Buffer, Self::Error> {
        Err(NoTransportError)
    }
}

fn flash_write(firmware: &[u8]) {
    let checksum = firmware
        .iter()
        .fold(0u32, |acc, &b| acc.wrapping_add(b as u32));

    hprintln!(
        "  writing {} bytes to flash (checksum: 0x{:08x})",
        firmware.len(),
        checksum
    );
}

#[entry]
fn main() -> ! {
    hprintln!("");
    hprintln!("╔══════════════════════════════════════════════╗");
    hprintln!("║  stuf v0.1.0 — TUF firmware updater         ║");
    hprintln!("║  profile: no-heap verifier                  ║");
    hprintln!("║  target:  ARM Cortex-M3 (lm3s6965evb)       ║");
    hprintln!("╚══════════════════════════════════════════════╝");
    hprintln!("");
    hprintln!("installed: firmware v{}", CURRENT_VERSION);
    hprintln!("root:      {} bytes (manufacture burn)", ROOT_BYTES.len());

    hprintln!("");
    hprintln!("━━━ NO-HEAP UPDATE SEQUENCE ━━━━━━━━━━━━━━━━━━");

    hprintln!("[1/5] fetching timestamp...");
    let ts = fetch_to_buf("timestamp.json", unsafe {
        static_buf(core::ptr::addr_of_mut!(TS_BUF))
    })
    .unwrap_or_else(|| {
        hprintln!("      FAILED");
        loop {}
    });
    hprintln!("      ok ({} bytes)", ts.len());

    hprintln!("[2/5] fetching snapshot...");
    let snap = fetch_to_buf("snapshot.json", unsafe {
        static_buf(core::ptr::addr_of_mut!(SNAP_BUF))
    })
    .unwrap_or_else(|| {
        hprintln!("      FAILED");
        loop {}
    });
    hprintln!("      ok ({} bytes)", snap.len());

    hprintln!("[3/5] fetching targets...");
    let targets = fetch_to_buf("targets.json", unsafe {
        static_buf(core::ptr::addr_of_mut!(TARGETS_BUF))
    })
    .unwrap_or_else(|| {
        hprintln!("      FAILED");
        loop {}
    });
    hprintln!("      ok ({} bytes)", targets.len());

    hprintln!("[4/5] verifying TUF metadata chain...");
    let clock = FixedClock(1_700_000_000);

    hprintln!("      starting root verification...");
    let anchor = TrustAnchor::new(ROOT_BYTES, NoTransport, clock).unwrap_or_else(|e| {
        hprintln!("  root FAILED: {:?}", e);
        loop {}
    });
    hprintln!("      root verified");

    let ts_ok = anchor.verify_timestamp_bytes(ts).unwrap_or_else(|e| {
        hprintln!("  timestamp FAILED: {:?}", e);
        loop {}
    });
    hprintln!("      timestamp verified");

    let snap_ok = ts_ok.verify_snapshot_bytes(snap).unwrap_or_else(|e| {
        hprintln!("  snapshot FAILED: {:?}", e);
        loop {}
    });
    hprintln!("      snapshot verified");

    let tgt_ok = snap_ok.verify_targets_bytes(targets).unwrap_or_else(|e| {
        hprintln!("  targets FAILED: {:?}", e);
        loop {}
    });
    hprintln!("      targets verified");

    hprintln!("[5/5] fetching firmware v{}...", NEW_VERSION);
    let firmware = fetch_to_buf(TARGET_FIRMWARE, unsafe {
        static_buf(core::ptr::addr_of_mut!(FIRMWARE_BUF_BYTES))
    })
    .unwrap_or_else(|| {
        hprintln!("      FAILED");
        loop {}
    });

    let _verified = tgt_ok
        .verify_target_bytes(TARGET_FIRMWARE, firmware)
        .unwrap_or_else(|e| {
            hprintln!("  firmware REJECTED: {:?}", e);
            loop {}
        });

    hprintln!("      firmware verified ({} bytes)", firmware.len());

    hprintln!("");
    hprintln!("update: v{} -> v{}", CURRENT_VERSION, NEW_VERSION);
    hprintln!("installing...");
    flash_write(firmware);
    hprintln!("ok firmware v{} installed (verified, no heap)", NEW_VERSION);
    hprintln!("");
    hprintln!("demo complete.");

    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
    loop {}
}
