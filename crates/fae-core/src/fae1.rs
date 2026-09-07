use crc32fast::Hasher;
use std::fmt;

pub const MAGIC_AND_VERSION: u32 = 0xFAEC_0D10;
pub const FIXED_WORD_COUNT: usize = 5;
pub const FIXED_SIZE: usize = FIXED_WORD_COUNT * 4;
pub const MAX_EXTRA_WORDS: usize = 15;

pub mod registry {
    pub mod isa {
        pub const ARM: u8 = 0x01;
        pub const RISCV: u8 = 0x02;
        pub const INTEL: u8 = 0x03;
        pub const PRIVATE_0: u8 = 0xFC;
        pub const PRIVATE_1: u8 = 0xFD;

        pub mod arm {
            // Only execution modes currently emitted by the in-tree builders.
            pub const A32: u8 = 0x01;
            pub const THUMB_2: u8 = 0x02;
            pub const THUMB_V6M: u8 = 0x03;
        }
    }

    pub mod abi {
        pub const XIPFS: u32 = 0x0FACADE1;
        pub const OXIDE_SE: u32 = 0x0AC1DA99;

        pub const RUSTLET_APPLICATION: u16 = 0xA99;
        pub const RUSTLET_SECURITY_DOMAIN: u16 = 0x5DC;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    TooManyExtraWords { kind: &'static str, count: usize },
    InvalidInlineBits(u16),
    InvalidAbiFamily(u32),
    InvalidExtraFieldsDescriptor(u32),
    Truncated,
    InvalidMagic(u32),
    FooterSizeOverflow,
    CrcMismatch { stored: u32, computed: u32 },
    WrongAbi { expected: u32, actual: u32 },
    WrongExtraWordCount { expected: usize, actual: usize },
    InvalidRustletProfile(u16),
    RustletMemoryRequirementTooLarge { kind: &'static str, bytes: u32 },
    InvalidRiscvIsa(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsaDescriptor {
    pub family: u8,
    pub subgroup: u8,
    pub inline_bits: u16,
    pub extra_word_count: u8,
}

impl IsaDescriptor {
    pub fn new(
        family: u8,
        subgroup: u8,
        inline_bits: u16,
        extra_word_count: usize,
    ) -> Result<Self, Error> {
        if inline_bits > 0x0FFF {
            return Err(Error::InvalidInlineBits(inline_bits));
        }
        let extra_word_count = checked_count("ISA", extra_word_count)?;
        Ok(Self {
            family,
            subgroup,
            inline_bits,
            extra_word_count,
        })
    }

    pub fn encode(self) -> u32 {
        (u32::from(self.family) << 24)
            | (u32::from(self.subgroup) << 16)
            | (u32::from(self.inline_bits) << 4)
            | u32::from(self.extra_word_count)
    }

    pub fn decode(word: u32) -> Self {
        Self {
            family: (word >> 24) as u8,
            subgroup: (word >> 16) as u8,
            inline_bits: ((word >> 4) & 0x0FFF) as u16,
            extra_word_count: (word & 0x0F) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiDescriptor {
    pub family: u32,
    pub extra_word_count: u8,
}

impl AbiDescriptor {
    pub fn new(family: u32, extra_word_count: usize) -> Result<Self, Error> {
        if family > 0x0FFF_FFFF {
            return Err(Error::InvalidAbiFamily(family));
        }
        Ok(Self {
            family,
            extra_word_count: checked_count("ABI", extra_word_count)?,
        })
    }

    pub fn encode(self) -> u32 {
        (self.family << 4) | u32::from(self.extra_word_count)
    }

    pub fn decode(word: u32) -> Self {
        Self {
            family: word >> 4,
            extra_word_count: (word & 0x0F) as u8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    pub isa: IsaDescriptor,
    pub abi: AbiDescriptor,
    pub extra_words: Vec<u32>,
    pub isa_extra_words: Vec<u32>,
    pub abi_extra_words: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedFooter {
    pub footer: Footer,
    pub footer_size: usize,
    pub stored_crc: u32,
}

impl Footer {
    pub fn new(
        family: u8,
        subgroup: u8,
        inline_bits: u16,
        extra_words: Vec<u32>,
        isa_extra_words: Vec<u32>,
        abi_family: u32,
        abi_extra_words: Vec<u32>,
    ) -> Result<Self, Error> {
        Ok(Self {
            isa: IsaDescriptor::new(family, subgroup, inline_bits, isa_extra_words.len())?,
            abi: AbiDescriptor::new(abi_family, abi_extra_words.len())?,
            extra_words,
            isa_extra_words,
            abi_extra_words,
        })
    }

    pub fn size(&self) -> Result<usize, Error> {
        footer_size(
            self.extra_words.len(),
            self.isa.extra_word_count,
            self.abi.extra_word_count,
        )
    }

    pub fn append_to(&self, image: &mut Vec<u8>) -> Result<(), Error> {
        validate_extra_lengths(self)?;
        let footer_size = self.size()?;
        image.reserve(footer_size);

        // Extras are physically reversed so backward reading yields logical order.
        for word in self.abi_extra_words.iter().rev() {
            push_word(image, *word);
        }
        for word in self.isa_extra_words.iter().rev() {
            push_word(image, *word);
        }
        for word in self.extra_words.iter().rev() {
            push_word(image, *word);
        }
        push_word(image, 0);
        push_word(
            image,
            u32::try_from(self.extra_words.len()).map_err(|_| Error::FooterSizeOverflow)?,
        );
        push_word(image, self.abi.encode());
        push_word(image, self.isa.encode());
        push_word(image, MAGIC_AND_VERSION);

        let crc_offset = image
            .len()
            .checked_sub(20)
            .ok_or(Error::FooterSizeOverflow)?;
        let crc = crc32_with_zeroed_field(image, crc_offset)?;
        image[crc_offset..crc_offset + 4].copy_from_slice(&crc.to_le_bytes());
        Ok(())
    }
}

pub fn decode(image: &[u8]) -> Result<DecodedFooter, Error> {
    if image.len() < FIXED_SIZE {
        return Err(Error::Truncated);
    }
    let magic = read_word_from_end(image, 4)?;
    if magic != MAGIC_AND_VERSION {
        return Err(Error::InvalidMagic(magic));
    }

    let isa = IsaDescriptor::decode(read_word_from_end(image, 8)?);
    let abi = AbiDescriptor::decode(read_word_from_end(image, 12)?);
    let extra_descriptor = read_word_from_end(image, 16)?;
    if extra_descriptor & 0xFFFF_FF00 != 0 {
        return Err(Error::InvalidExtraFieldsDescriptor(extra_descriptor));
    }
    let extra_count = (extra_descriptor & 0xFF) as usize;
    let footer_size = footer_size(extra_count, isa.extra_word_count, abi.extra_word_count)?;
    if footer_size > image.len() {
        return Err(Error::Truncated);
    }

    let stored_crc = read_word_from_end(image, 20)?;
    let crc_offset = image.len() - 20;
    let computed_crc = crc32_with_zeroed_field(image, crc_offset)?;
    if stored_crc != computed_crc {
        return Err(Error::CrcMismatch {
            stored: stored_crc,
            computed: computed_crc,
        });
    }

    let mut distance = 24usize;
    let mut extra_words = Vec::with_capacity(extra_count);
    for _ in 0..extra_count {
        extra_words.push(read_word_from_end(image, distance)?);
        distance = distance.checked_add(4).ok_or(Error::FooterSizeOverflow)?;
    }
    let mut isa_extra_words = Vec::with_capacity(usize::from(isa.extra_word_count));
    for _ in 0..isa.extra_word_count {
        isa_extra_words.push(read_word_from_end(image, distance)?);
        distance = distance.checked_add(4).ok_or(Error::FooterSizeOverflow)?;
    }
    let mut abi_extra_words = Vec::with_capacity(usize::from(abi.extra_word_count));
    for _ in 0..abi.extra_word_count {
        abi_extra_words.push(read_word_from_end(image, distance)?);
        distance = distance.checked_add(4).ok_or(Error::FooterSizeOverflow)?;
    }

    Ok(DecodedFooter {
        footer: Footer {
            isa,
            abi,
            extra_words,
            isa_extra_words,
            abi_extra_words,
        },
        footer_size,
        stored_crc,
    })
}

fn checked_count(kind: &'static str, count: usize) -> Result<u8, Error> {
    if count > MAX_EXTRA_WORDS {
        return Err(Error::TooManyExtraWords { kind, count });
    }
    u8::try_from(count).map_err(|_| Error::TooManyExtraWords { kind, count })
}

fn footer_size(extra_count: usize, isa_count: u8, abi_count: u8) -> Result<usize, Error> {
    FIXED_WORD_COUNT
        .checked_add(extra_count)
        .and_then(|v| v.checked_add(usize::from(isa_count)))
        .and_then(|v| v.checked_add(usize::from(abi_count)))
        .and_then(|v| v.checked_mul(4))
        .ok_or(Error::FooterSizeOverflow)
}

fn validate_extra_lengths(footer: &Footer) -> Result<(), Error> {
    if usize::from(footer.isa.extra_word_count) != footer.isa_extra_words.len() {
        return Err(Error::WrongExtraWordCount {
            expected: usize::from(footer.isa.extra_word_count),
            actual: footer.isa_extra_words.len(),
        });
    }
    if usize::from(footer.abi.extra_word_count) != footer.abi_extra_words.len() {
        return Err(Error::WrongExtraWordCount {
            expected: usize::from(footer.abi.extra_word_count),
            actual: footer.abi_extra_words.len(),
        });
    }
    Ok(())
}

fn push_word(out: &mut Vec<u8>, word: u32) {
    out.extend_from_slice(&word.to_le_bytes());
}

fn read_word_from_end(image: &[u8], distance: usize) -> Result<u32, Error> {
    let start = image.len().checked_sub(distance).ok_or(Error::Truncated)?;
    let end = start.checked_add(4).ok_or(Error::FooterSizeOverflow)?;
    let bytes: [u8; 4] = image
        .get(start..end)
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u32::from_le_bytes(bytes))
}

pub fn crc32_with_zeroed_field(image: &[u8], crc_offset: usize) -> Result<u32, Error> {
    let crc_end = crc_offset.checked_add(4).ok_or(Error::FooterSizeOverflow)?;
    if crc_end > image.len() {
        return Err(Error::Truncated);
    }
    let mut hasher = Hasher::new();
    hasher.update(&image[..crc_offset]);
    hasher.update(&[0; 4]);
    hasher.update(&image[crc_end..]);
    Ok(hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XipfsProfile {
    pub ram_size: u32,
    pub got_size: u32,
    pub rom_size: u32,
    pub rom_ram_size: u32,
    pub entrypoint_offset: u32,
    pub startup_code_size: u32,
}

impl XipfsProfile {
    pub const EXTRA_WORD_COUNT: usize = 6;

    pub fn words(self) -> Vec<u32> {
        vec![
            self.ram_size,
            self.got_size,
            self.rom_size,
            self.rom_ram_size,
            self.entrypoint_offset,
            self.startup_code_size,
        ]
    }

    pub fn decode(footer: &Footer) -> Result<Self, Error> {
        require_profile(footer, registry::abi::XIPFS, Self::EXTRA_WORD_COUNT)?;
        let w = &footer.abi_extra_words;
        Ok(Self {
            ram_size: w[0],
            got_size: w[1],
            rom_size: w[2],
            rom_ram_size: w[3],
            entrypoint_offset: w[4],
            startup_code_size: w[5],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub revision: u8,
}

impl AbiVersion {
    pub fn encode(self) -> Result<u32, Error> {
        if self.minor > 0x0F || self.patch > 0x0F || self.revision > 0x0F {
            return Err(Error::InvalidRustletProfile(0));
        }
        Ok((u32::from(self.major) << 12)
            | (u32::from(self.minor) << 8)
            | (u32::from(self.patch) << 4)
            | u32::from(self.revision))
    }

    pub fn decode(word: u32) -> Self {
        Self {
            major: ((word >> 12) & 0xFF) as u8,
            minor: ((word >> 8) & 0x0F) as u8,
            patch: ((word >> 4) & 0x0F) as u8,
            revision: (word & 0x0F) as u8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustletProfileKind {
    Application,
    SecurityDomain,
}

impl RustletProfileKind {
    fn marker(self) -> u16 {
        match self {
            Self::Application => registry::abi::RUSTLET_APPLICATION,
            Self::SecurityDomain => registry::abi::RUSTLET_SECURITY_DOMAIN,
        }
    }

    fn from_marker(marker: u16) -> Result<Self, Error> {
        match marker {
            registry::abi::RUSTLET_APPLICATION => Ok(Self::Application),
            registry::abi::RUSTLET_SECURITY_DOMAIN => Ok(Self::SecurityDomain),
            _ => Err(Error::InvalidRustletProfile(marker)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OxideSeProfile {
    pub kind: RustletProfileKind,
    pub minimum_version: AbiVersion,
    pub writable_ram_size: u32,
    pub stack_size: u32,
}

impl OxideSeProfile {
    pub const EXTRA_WORD_COUNT: usize = 2;
    pub const MEMORY_UNIT: u32 = 32;
    pub const MAX_MEMORY_REQUIREMENT: u32 = u16::MAX as u32 * Self::MEMORY_UNIT;

    pub fn words(self) -> Result<Vec<u32>, Error> {
        let profile_version =
            (u32::from(self.kind.marker()) << 20) | self.minimum_version.encode()?;
        let writable_ram_units = encode_memory_units("writable RAM", self.writable_ram_size)?;
        let stack_units = encode_memory_units("stack", self.stack_size)?;
        Ok(vec![
            profile_version,
            (u32::from(writable_ram_units) << 16) | u32::from(stack_units),
        ])
    }

    pub fn decode(footer: &Footer) -> Result<Self, Error> {
        require_profile(footer, registry::abi::OXIDE_SE, Self::EXTRA_WORD_COUNT)?;
        let w = &footer.abi_extra_words;
        let marker = (w[0] >> 20) as u16;
        Ok(Self {
            kind: RustletProfileKind::from_marker(marker)?,
            minimum_version: AbiVersion::decode(w[0] & 0x000F_FFFF),
            writable_ram_size: ((w[1] >> 16) & 0xFFFF) * Self::MEMORY_UNIT,
            stack_size: (w[1] & 0xFFFF) * Self::MEMORY_UNIT,
        })
    }
}

fn encode_memory_units(kind: &'static str, bytes: u32) -> Result<u16, Error> {
    let units = bytes
        .checked_add(OxideSeProfile::MEMORY_UNIT - 1)
        .ok_or(Error::RustletMemoryRequirementTooLarge { kind, bytes })?
        / OxideSeProfile::MEMORY_UNIT;
    u16::try_from(units).map_err(|_| Error::RustletMemoryRequirementTooLarge { kind, bytes })
}

fn require_profile(footer: &Footer, family: u32, words: usize) -> Result<(), Error> {
    if footer.abi.family != family {
        return Err(Error::WrongAbi {
            expected: family,
            actual: footer.abi.family,
        });
    }
    if footer.abi_extra_words.len() != words {
        return Err(Error::WrongExtraWordCount {
            expected: words,
            actual: footer.abi_extra_words.len(),
        });
    }
    Ok(())
}

pub fn encode_riscv_isa(isa: &str) -> Result<Vec<u32>, Error> {
    if !(isa.starts_with("rv32") || isa.starts_with("rv64")) {
        return Err(Error::InvalidRiscvIsa("must start with rv32 or rv64"));
    }
    if !isa
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(Error::InvalidRiscvIsa("must use canonical lowercase ASCII"));
    }
    let byte_len = isa.len().checked_add(1).ok_or(Error::FooterSizeOverflow)?;
    let word_count = byte_len.checked_add(3).ok_or(Error::FooterSizeOverflow)? / 4;
    checked_count("ISA", word_count)?;
    let mut bytes = vec![0; word_count * 4];
    bytes[..isa.len()].copy_from_slice(isa.as_bytes());
    let (words, remainder) = bytes.as_chunks::<4>();
    debug_assert!(remainder.is_empty());
    Ok(words.iter().map(|word| u32::from_le_bytes(*word)).collect())
}

pub fn decode_riscv_isa(words: &[u32]) -> Result<String, Error> {
    if words.is_empty() || words.len() > MAX_EXTRA_WORDS {
        return Err(Error::InvalidRiscvIsa("invalid word count"));
    }
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let nul = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::InvalidRiscvIsa("missing NUL terminator"))?;
    if bytes[nul + 1..].iter().any(|byte| *byte != 0) {
        return Err(Error::InvalidRiscvIsa("non-zero byte after terminator"));
    }
    let text =
        std::str::from_utf8(&bytes[..nul]).map_err(|_| Error::InvalidRiscvIsa("not UTF-8"))?;
    encode_riscv_isa(text)?;
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm_xipfs_footer() -> (Footer, XipfsProfile) {
        let profile = XipfsProfile {
            ram_size: 12,
            got_size: 8,
            rom_size: 100,
            rom_ram_size: 20,
            entrypoint_offset: 5,
            startup_code_size: 64,
        };
        let footer = Footer::new(
            registry::isa::ARM,
            registry::isa::arm::THUMB_2,
            0,
            vec![],
            vec![],
            registry::abi::XIPFS,
            profile.words(),
        )
        .unwrap();
        (footer, profile)
    }

    #[test]
    fn xipfs_round_trip_and_wire_order() {
        let (footer, profile) = arm_xipfs_footer();
        let mut image = b"payload".to_vec();
        footer.append_to(&mut image).unwrap();
        let decoded = decode(&image).unwrap();
        assert_eq!(decoded.footer_size, 44);
        assert_eq!(decoded.footer, footer);
        assert_eq!(XipfsProfile::decode(&decoded.footer).unwrap(), profile);
        assert_eq!(&image[image.len() - 4..], &MAGIC_AND_VERSION.to_le_bytes());
        assert_eq!(read_word_from_end(&image, 24).unwrap(), 12);
        assert_eq!(read_word_from_end(&image, 44).unwrap(), 64);
    }

    #[test]
    fn crc_covers_payload_fixed_and_additional_words() {
        let (footer, _) = arm_xipfs_footer();
        let mut image = b"payload".to_vec();
        footer.append_to(&mut image).unwrap();
        for index in [0, image.len() - 4, image.len() - 24, image.len() - 44] {
            let mut corrupted = image.clone();
            corrupted[index] ^= 0x80;
            assert!(matches!(
                decode(&corrupted),
                Err(Error::CrcMismatch { .. }) | Err(Error::InvalidMagic(_))
            ));
        }
    }

    #[test]
    fn rejects_truncated_and_oversized_extensions() {
        assert_eq!(decode(&[0; 12]), Err(Error::Truncated));
        assert!(matches!(
            Footer::new(1, 1, 0, vec![], vec![0; 16], 1, vec![]),
            Err(Error::TooManyExtraWords { .. })
        ));
    }

    #[test]
    fn rustlet_profiles_round_trip() {
        for kind in [
            RustletProfileKind::Application,
            RustletProfileKind::SecurityDomain,
        ] {
            let profile = OxideSeProfile {
                kind,
                minimum_version: AbiVersion {
                    major: 1,
                    minor: 2,
                    patch: 3,
                    revision: 4,
                },
                writable_ram_size: 1024,
                stack_size: 513,
            };
            let footer = Footer::new(
                1,
                2,
                0,
                vec![],
                vec![],
                registry::abi::OXIDE_SE,
                profile.words().unwrap(),
            )
            .unwrap();
            assert_eq!(
                OxideSeProfile::decode(&footer).unwrap(),
                OxideSeProfile {
                    stack_size: 544,
                    ..profile
                }
            );
        }
    }

    #[test]
    fn rustlet_profile_rejects_memory_larger_than_encodable_limit() {
        let profile = OxideSeProfile {
            kind: RustletProfileKind::Application,
            minimum_version: AbiVersion {
                major: 1,
                minor: 0,
                patch: 0,
                revision: 0,
            },
            writable_ram_size: OxideSeProfile::MAX_MEMORY_REQUIREMENT + 1,
            stack_size: 2048,
        };
        assert!(matches!(
            profile.words(),
            Err(Error::RustletMemoryRequirementTooLarge {
                kind: "writable RAM",
                ..
            })
        ));
    }

    #[test]
    fn riscv_canonical_text_round_trip() {
        let words = encode_riscv_isa("rv32imac_zicsr").unwrap();
        assert_eq!(decode_riscv_isa(&words).unwrap(), "rv32imac_zicsr");
        assert!(encode_riscv_isa("RV32I").is_err());
        assert!(encode_riscv_isa(&format!("rv32{}", "i".repeat(60))).is_err());
    }
}
