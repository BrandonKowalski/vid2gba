use anyhow::{Context, Result, ensure};

pub const PLAYER: &[u8] = include_bytes!("../../player/player.gba");
pub const MARKER: &[u8; 8] = b"VGBAPTR\0";
pub const ROM_BASE: u32 = 0x0800_0000;

const LOGO: [u8; 156] = [
    0x24, 0xFF, 0xAE, 0x51, 0x69, 0x9A, 0xA2, 0x21, 0x3D, 0x84, 0x82, 0x0A,
    0x84, 0xE4, 0x09, 0xAD, 0x11, 0x24, 0x8B, 0x98, 0xC0, 0x81, 0x7F, 0x21,
    0xA3, 0x52, 0xBE, 0x19, 0x93, 0x09, 0xCE, 0x20, 0x10, 0x46, 0x4A, 0x4A,
    0xF8, 0x27, 0x31, 0xEC, 0x58, 0xC7, 0xE8, 0x33, 0x82, 0xE3, 0xCE, 0xBF,
    0x85, 0xF4, 0xDF, 0x94, 0xCE, 0x4B, 0x09, 0xC1, 0x94, 0x56, 0x8A, 0xC0,
    0x13, 0x72, 0xA7, 0xFC, 0x9F, 0x84, 0x4D, 0x73, 0xA3, 0xCA, 0x9A, 0x61,
    0x58, 0x97, 0xA3, 0x27, 0xFC, 0x03, 0x98, 0x76, 0x23, 0x1D, 0xC7, 0x61,
    0x03, 0x04, 0xAE, 0x56, 0xBF, 0x38, 0x84, 0x00, 0x40, 0xA7, 0x0E, 0xFD,
    0xFF, 0x52, 0xFE, 0x03, 0x6F, 0x95, 0x30, 0xF1, 0x97, 0xFB, 0xC0, 0x85,
    0x60, 0xD6, 0x80, 0x25, 0xA9, 0x63, 0xBE, 0x03, 0x01, 0x4E, 0x38, 0xE2,
    0xF9, 0xA2, 0x34, 0xFF, 0xBB, 0x3E, 0x03, 0x44, 0x78, 0x00, 0x90, 0xCB,
    0x88, 0x11, 0x3A, 0x94, 0x65, 0xC0, 0x7C, 0x63, 0x87, 0xF0, 0x3C, 0xAF,
    0xD6, 0x25, 0xE4, 0x8B, 0x38, 0x0A, 0xAC, 0x72, 0x21, 0xD4, 0xF8, 0x07,
];

fn find_marker(rom: &[u8]) -> Option<usize> {
    rom.windows(8).position(|w| w == MARKER)
}

pub fn header_checksum(rom: &[u8]) -> u8 {
    rom[0xA0..0xBD].iter().fold(0u8, |c, &b| c.wrapping_sub(b)).wrapping_sub(0x19)
}

pub fn assemble(player: &[u8], container: &[u8], title: &str) -> Result<Vec<u8>> {
    ensure!(player.len() >= 0xC0, "player ROM is too small");
    let slot = find_marker(player).context("player ROM has no container slot")?;
    let mut rom = player.to_vec();
    rom.resize(rom.len().div_ceil(256) * 256, 0);
    let addr = ROM_BASE + rom.len() as u32;
    rom[slot + 8..slot + 12].copy_from_slice(&addr.to_le_bytes());
    rom.extend_from_slice(container);
    rom[0x04..0xA0].copy_from_slice(&LOGO);
    let mut t = [0u8; 12];
    for (i, c) in title.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').take(12).enumerate() {
        t[i] = c.to_ascii_uppercase() as u8;
    }
    rom[0xA0..0xAC].copy_from_slice(&t);
    rom[0xAC..0xB0].copy_from_slice(b"VGBA");
    rom[0xB0..0xB2].copy_from_slice(b"00");
    rom[0xB2] = 0x96;
    rom[0xBD] = header_checksum(&rom);
    Ok(rom)
}

pub fn container_of(rom: &[u8]) -> Result<&[u8]> {
    let slot = find_marker(rom).context("ROM has no container slot")?;
    let addr = u32::from_le_bytes(rom[slot + 8..slot + 12].try_into().unwrap());
    ensure!(addr >= ROM_BASE && ((addr - ROM_BASE) as usize) < rom.len(), "container slot is not set");
    Ok(&rom[(addr - ROM_BASE) as usize..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_player() -> Vec<u8> {
        let mut p = vec![0u8; 0x1F0];
        p[0x100..0x108].copy_from_slice(MARKER);
        p
    }

    #[test]
    fn logo_looks_right() {
        assert_eq!(&LOGO[..4], &[0x24, 0xFF, 0xAE, 0x51]);
    }

    #[test]
    fn assembles_and_patches_slot() {
        let container = vec![7u8; 100];
        let rom = assemble(&fake_player(), &container, "Never Gonna Give").unwrap();
        assert_eq!(rom.len(), 0x200 + 100);
        assert_eq!(u32::from_le_bytes(rom[0x108..0x10C].try_into().unwrap()), 0x0800_0200);
        assert_eq!(&rom[0x200..], &container[..]);
        assert_eq!(&rom[0xA0..0xAC], b"NEVER GONNA ");
        assert_eq!(&rom[0xAC..0xB0], b"VGBA");
        assert_eq!(rom[0xB2], 0x96);
        assert_eq!(&rom[0x04..0xA0], &LOGO[..]);
        let sum = rom[0xA0..=0xBD].iter().fold(0u8, |a, &b| a.wrapping_add(b)).wrapping_add(0x19);
        assert_eq!(sum, 0);
        assert_eq!(container_of(&rom).unwrap(), &container[..]);
    }

    #[test]
    fn title_header_filters_symbols() {
        let rom = assemble(&fake_player(), &[], "Café ☕!").unwrap();
        assert_eq!(&rom[0xA0..0xAC], b"CAF \0\0\0\0\0\0\0\0");
    }

    #[test]
    fn missing_marker_is_an_error() {
        assert!(assemble(&vec![0u8; 0x200], &[], "x").is_err());
        assert!(container_of(&[0u8; 0x200]).is_err());
    }

    #[test]
    fn embedded_player_has_marker() {
        assert_eq!(PLAYER.windows(8).filter(|w| w == MARKER).count(), 1);
    }
}
