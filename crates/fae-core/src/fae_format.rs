use crate::common::{round_up_pow2, to_word};
use crate::constants;
use crate::fae1::{self, AbiVersion, Footer, OxideSeProfile, RustletProfileKind, XipfsProfile};

#[derive(Debug, Clone, Copy)]
pub struct ExportedSymbols {
    pub start: u32,
    pub rom_ram_size: u32,
    pub rom_size: u32,
    pub got_size: u32,
    pub ram_size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Facade12,
    Fae1 { isa: DescriptorWords, abi: Fae1Abi },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorWords {
    descriptor: u32,
    words: [u32; fae1::MAX_EXTRA_WORDS],
    count: u8,
}

impl DescriptorWords {
    pub fn new(descriptor: u32, words: &[u32]) -> Result<Self, fae1::Error> {
        let expected = (descriptor & 0x0F) as usize;
        if expected != words.len() {
            return Err(fae1::Error::WrongExtraWordCount {
                expected,
                actual: words.len(),
            });
        }
        if words.len() > fae1::MAX_EXTRA_WORDS {
            return Err(fae1::Error::TooManyExtraWords {
                kind: "descriptor",
                count: words.len(),
            });
        }
        let mut stored = [0; fae1::MAX_EXTRA_WORDS];
        stored[..words.len()].copy_from_slice(words);
        Ok(Self {
            descriptor,
            words: stored,
            count: words.len() as u8,
        })
    }

    pub fn arm(subgroup: u8) -> Self {
        Self {
            descriptor: fae1::IsaDescriptor {
                family: fae1::registry::isa::ARM,
                subgroup,
                inline_bits: 0,
                extra_word_count: 0,
            }
            .encode(),
            words: [0; fae1::MAX_EXTRA_WORDS],
            count: 0,
        }
    }

    pub fn descriptor(self) -> u32 {
        self.descriptor
    }

    fn to_vec(self) -> Vec<u32> {
        self.words[..usize::from(self.count)].to_vec()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fae1Abi {
    Xipfs,
    Rustlet {
        kind: RustletProfileKind,
        minimum_version: AbiVersion,
        stack_size: u32,
    },
    Explicit(DescriptorWords),
}

impl Fae1Abi {
    pub fn uses_rustlet_layout(self) -> bool {
        match self {
            Self::Rustlet { .. } => true,
            Self::Explicit(words) => {
                fae1::AbiDescriptor::decode(words.descriptor()).family
                    == fae1::registry::abi::OXIDE_SE
            }
            Self::Xipfs => false,
        }
    }
}

impl OutputFormat {
    fn private_metadata_size(self) -> usize {
        match self {
            Self::Fae1 { abi, .. } if abi.uses_rustlet_layout() => 5 * 4,
            _ => 0,
        }
    }
}

pub fn compute_header_size(startup_code: &[u8], relocation_bytearray: &[u8]) -> usize {
    startup_code.len() + constants::BINARY_SIZE_BYTESIZE + relocation_bytearray.len()
}

pub fn compute_payload_alignment(
    startup_code: &[u8],
    relocation_bytearray: &[u8],
    align_payload: usize,
) -> usize {
    if align_payload <= 1 {
        return 0;
    }

    let unaligned_partition_offset = compute_header_size(startup_code, relocation_bytearray)
        + constants::PAYLOAD_PADDING_BYTESIZE;
    round_up_pow2(unaligned_partition_offset, align_payload) - unaligned_partition_offset
}

pub fn compute_text_alignment(
    startup_code: &[u8],
    relocation_bytearray: &[u8],
    align_payload: usize,
) -> usize {
    compute_payload_alignment(startup_code, relocation_bytearray, align_payload)
}

pub fn compute_aligned_header_size(
    startup_code: &[u8],
    relocation_bytearray: &[u8],
    align_payload: usize,
) -> usize {
    compute_header_size(startup_code, relocation_bytearray)
        + constants::PAYLOAD_PADDING_BYTESIZE
        + compute_payload_alignment(startup_code, relocation_bytearray, align_payload)
}

pub fn compute_aligned_header_size_for_format(
    startup_code: &[u8],
    relocation_bytearray: &[u8],
    align_payload: usize,
    format: OutputFormat,
) -> usize {
    let base = compute_header_size(startup_code, relocation_bytearray)
        + format.private_metadata_size()
        + constants::PAYLOAD_PADDING_BYTESIZE;
    if align_payload <= 1 {
        base
    } else {
        round_up_pow2(base, align_payload)
    }
}

pub fn pad_section_size(size: u32) -> u32 {
    round_up_pow2(size as usize, constants::SECTION_ALIGNMENT) as u32
}

pub fn padded_exported_symbols(symbols: ExportedSymbols) -> ExportedSymbols {
    ExportedSymbols {
        start: symbols.start,
        rom_ram_size: pad_section_size(symbols.rom_ram_size),
        rom_size: pad_section_size(symbols.rom_size),
        got_size: pad_section_size(symbols.got_size),
        ram_size: pad_section_size(symbols.ram_size),
    }
}

pub fn remap_offset_to_padded_layout(
    offset: u32,
    raw: ExportedSymbols,
    padded: ExportedSymbols,
) -> Option<u32> {
    if offset < raw.rom_size {
        return Some(offset);
    }

    let got_start = raw.rom_size;
    let got_end = got_start.checked_add(raw.got_size)?;
    if offset < got_end {
        return padded.rom_size.checked_add(offset.checked_sub(got_start)?);
    }

    let rom_ram_start = got_end;
    let rom_ram_end = rom_ram_start.checked_add(raw.rom_ram_size)?;
    if offset < rom_ram_end {
        return padded
            .rom_size
            .checked_add(padded.got_size)?
            .checked_add(offset.checked_sub(rom_ram_start)?);
    }

    let ram_start = rom_ram_end;
    let ram_end = ram_start.checked_add(raw.ram_size)?;
    if offset < ram_end {
        return padded
            .rom_size
            .checked_add(padded.got_size)?
            .checked_add(padded.rom_ram_size)?
            .checked_add(offset.checked_sub(ram_start)?);
    }

    None
}

pub fn concatenate_and_pad_bytearray(
    startup_code: &[u8],
    exported_symbols: ExportedSymbols,
    relocation_bytearray: &[u8],
    align_payload: usize,
    align_size: usize,
    partition_bytearray: &[u8],
) -> Vec<u8> {
    concatenate_legacy(
        startup_code,
        exported_symbols,
        relocation_bytearray,
        align_payload,
        align_size,
        partition_bytearray,
    )
}

pub fn concatenate_and_pad_bytearray_for_format(
    startup_code: &[u8],
    exported_symbols: ExportedSymbols,
    relocation_bytearray: &[u8],
    align_payload: usize,
    align_size: usize,
    partition_bytearray: &[u8],
    format: OutputFormat,
) -> Result<Vec<u8>, fae1::Error> {
    match format {
        OutputFormat::Facade12 => Ok(concatenate_legacy(
            startup_code,
            exported_symbols,
            relocation_bytearray,
            align_payload,
            align_size,
            partition_bytearray,
        )),
        OutputFormat::Fae1 { isa, abi } => concatenate_fae1(
            startup_code,
            exported_symbols,
            relocation_bytearray,
            align_payload,
            align_size,
            partition_bytearray,
            isa,
            abi,
        ),
    }
}

fn concatenate_legacy(
    startup_code: &[u8],
    exported_symbols: ExportedSymbols,
    relocation_bytearray: &[u8],
    align_payload: usize,
    align_size: usize,
    partition_bytearray: &[u8],
) -> Vec<u8> {
    let payload_alignment =
        compute_payload_alignment(startup_code, relocation_bytearray, align_payload);
    let header_size =
        compute_aligned_header_size(startup_code, relocation_bytearray, align_payload);
    let raw_binary_size = header_size + partition_bytearray.len();

    let file_alignment = align_size.max(1);
    let aligned_binary_size =
        round_up_pow2(raw_binary_size + constants::FOOTER_BYTESIZE, file_alignment);
    let padding_size = aligned_binary_size - raw_binary_size - constants::FOOTER_BYTESIZE;

    let mut out = Vec::with_capacity(raw_binary_size + padding_size + constants::FOOTER_BYTESIZE);

    out.extend_from_slice(startup_code);
    out.extend_from_slice(&to_word(
        (raw_binary_size + padding_size + constants::FOOTER_BYTESIZE) as u32,
    ));
    out.extend_from_slice(relocation_bytearray);
    out.extend_from_slice(&to_word(payload_alignment as u32));
    out.extend(std::iter::repeat_n(
        constants::TEXT_ALIGNMENT_PADDING_VALUE,
        payload_alignment,
    ));
    out.extend_from_slice(partition_bytearray);
    out.extend(std::iter::repeat_n(constants::PADDING_VALUE, padding_size));

    out.extend_from_slice(&to_word(exported_symbols.ram_size));
    out.extend_from_slice(&to_word(exported_symbols.got_size));
    out.extend_from_slice(&to_word(exported_symbols.rom_size));
    out.extend_from_slice(&to_word(exported_symbols.rom_ram_size));
    out.extend_from_slice(&to_word(exported_symbols.start));
    out.extend_from_slice(&to_word(startup_code.len() as u32));
    out.extend_from_slice(&to_word(constants::MAGIC_NUMBER_AND_VERSION));

    out
}

#[allow(clippy::too_many_arguments)]
fn concatenate_fae1(
    startup_code: &[u8],
    exported_symbols: ExportedSymbols,
    relocation_bytearray: &[u8],
    align_payload: usize,
    align_size: usize,
    partition_bytearray: &[u8],
    isa: DescriptorWords,
    abi: Fae1Abi,
) -> Result<Vec<u8>, fae1::Error> {
    let (abi_family, abi_words) = match abi {
        Fae1Abi::Xipfs => {
            let profile = XipfsProfile {
                ram_size: exported_symbols.ram_size,
                got_size: exported_symbols.got_size,
                rom_size: exported_symbols.rom_size,
                rom_ram_size: exported_symbols.rom_ram_size,
                entrypoint_offset: exported_symbols.start,
                startup_code_size: u32::try_from(startup_code.len())
                    .map_err(|_| fae1::Error::FooterSizeOverflow)?,
            };
            (fae1::registry::abi::XIPFS, profile.words())
        }
        Fae1Abi::Rustlet {
            kind,
            minimum_version,
            stack_size,
        } => {
            let writable_ram_size = exported_symbols
                .got_size
                .checked_add(exported_symbols.rom_ram_size)
                .and_then(|size| size.checked_add(exported_symbols.ram_size))
                .ok_or(fae1::Error::FooterSizeOverflow)?;
            let profile = OxideSeProfile {
                kind,
                minimum_version,
                writable_ram_size,
                stack_size,
            };
            (fae1::registry::abi::OXIDE_SE, profile.words()?)
        }
        Fae1Abi::Explicit(words) => {
            let descriptor = fae1::AbiDescriptor::decode(words.descriptor());
            (descriptor.family, words.to_vec())
        }
    };
    let isa_descriptor = fae1::IsaDescriptor::decode(isa.descriptor());
    let footer = Footer::new(
        isa_descriptor.family,
        isa_descriptor.subgroup,
        isa_descriptor.inline_bits,
        vec![],
        isa.to_vec(),
        abi_family,
        abi_words,
    )?;
    let footer_size = footer.size()?;
    let private_metadata_size = OutputFormat::Fae1 { isa, abi }.private_metadata_size();
    let base_header_size = compute_header_size(startup_code, relocation_bytearray)
        .checked_add(private_metadata_size)
        .ok_or(fae1::Error::FooterSizeOverflow)?;
    let payload_alignment = if align_payload <= 1 {
        0
    } else {
        let unaligned_partition_offset = base_header_size
            .checked_add(constants::PAYLOAD_PADDING_BYTESIZE)
            .ok_or(fae1::Error::FooterSizeOverflow)?;
        round_up_pow2(unaligned_partition_offset, align_payload) - unaligned_partition_offset
    };
    let header_size = base_header_size
        .checked_add(constants::PAYLOAD_PADDING_BYTESIZE)
        .and_then(|size| size.checked_add(payload_alignment))
        .ok_or(fae1::Error::FooterSizeOverflow)?;
    let raw_binary_size = header_size
        .checked_add(partition_bytearray.len())
        .ok_or(fae1::Error::FooterSizeOverflow)?;
    let unaligned_size = raw_binary_size
        .checked_add(footer_size)
        .ok_or(fae1::Error::FooterSizeOverflow)?;
    let file_alignment = align_size.max(1);
    let aligned_binary_size = round_up_pow2(unaligned_size, file_alignment);
    let padding_size = aligned_binary_size
        .checked_sub(unaligned_size)
        .ok_or(fae1::Error::FooterSizeOverflow)?;
    let binary_size =
        u32::try_from(aligned_binary_size).map_err(|_| fae1::Error::FooterSizeOverflow)?;

    let mut out = Vec::with_capacity(aligned_binary_size);
    out.extend_from_slice(startup_code);
    out.extend_from_slice(&to_word(binary_size));
    if abi.uses_rustlet_layout() {
        // These words are private RT0 metadata, not part of the public FAE ABI.
        out.extend_from_slice(&to_word(exported_symbols.ram_size));
        out.extend_from_slice(&to_word(exported_symbols.got_size));
        out.extend_from_slice(&to_word(exported_symbols.rom_size));
        out.extend_from_slice(&to_word(exported_symbols.rom_ram_size));
        out.extend_from_slice(&to_word(exported_symbols.start));
    }
    out.extend_from_slice(relocation_bytearray);
    out.extend_from_slice(&to_word(
        u32::try_from(payload_alignment).map_err(|_| fae1::Error::FooterSizeOverflow)?,
    ));
    out.extend(std::iter::repeat_n(
        constants::TEXT_ALIGNMENT_PADDING_VALUE,
        payload_alignment,
    ));
    out.extend_from_slice(partition_bytearray);
    out.extend(std::iter::repeat_n(constants::PADDING_VALUE, padding_size));
    footer.append_to(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols() -> ExportedSymbols {
        ExportedSymbols {
            start: 1,
            rom_ram_size: 8,
            rom_size: 12,
            got_size: 4,
            ram_size: 16,
        }
    }

    #[test]
    fn legacy_selector_is_bit_for_bit_identical() {
        let startup = [1, 2, 3, 4];
        let relocations = 0u32.to_le_bytes();
        let partition = [0xAA; 24];
        let old =
            concatenate_and_pad_bytearray(&startup, symbols(), &relocations, 4, 32, &partition);
        let selected = concatenate_and_pad_bytearray_for_format(
            &startup,
            symbols(),
            &relocations,
            4,
            32,
            &partition,
            OutputFormat::Facade12,
        )
        .unwrap();
        assert_eq!(selected, old);
    }

    #[test]
    fn fae1_binary_size_and_profile_match_image() {
        let startup = [1, 2, 3, 4];
        let relocations = 0u32.to_le_bytes();
        let image = concatenate_and_pad_bytearray_for_format(
            &startup,
            symbols(),
            &relocations,
            4,
            32,
            &[0xAA; 24],
            OutputFormat::Fae1 {
                isa: DescriptorWords::arm(fae1::registry::isa::arm::THUMB_2),
                abi: Fae1Abi::Xipfs,
            },
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(image[4..8].try_into().unwrap()) as usize,
            image.len()
        );
        assert_eq!(image.len() % 32, 0);
        let decoded = fae1::decode(&image).unwrap();
        assert_eq!(XipfsProfile::decode(&decoded.footer).unwrap().rom_size, 12);
    }

    #[test]
    fn rustlet_fae1_uses_compact_public_profile_and_private_rt0_metadata() {
        let startup = [1, 2, 3, 4];
        let relocations = 0u32.to_le_bytes();
        let image = concatenate_and_pad_bytearray_for_format(
            &startup,
            symbols(),
            &relocations,
            4,
            1,
            &[0xAA; 24],
            OutputFormat::Fae1 {
                isa: DescriptorWords::arm(fae1::registry::isa::arm::THUMB_2),
                abi: Fae1Abi::Rustlet {
                    kind: RustletProfileKind::Application,
                    minimum_version: AbiVersion {
                        major: 1,
                        minor: 0,
                        patch: 0,
                        revision: 0,
                    },
                    stack_size: 2048,
                },
            },
        )
        .unwrap();
        let decoded = fae1::decode(&image).unwrap();
        assert_eq!(decoded.footer_size, 28);
        assert_eq!(decoded.footer.abi.encode(), 0xAC1D_A992);
        let profile = OxideSeProfile::decode(&decoded.footer).unwrap();
        assert_eq!(profile.writable_ram_size, 32);
        assert_eq!(profile.stack_size, 2048);
        assert_eq!(u32::from_le_bytes(image[8..12].try_into().unwrap()), 16);
        assert_eq!(u32::from_le_bytes(image[24..28].try_into().unwrap()), 1);
    }

    #[test]
    fn rustlet_alias_and_explicit_descriptors_are_identical() {
        let startup = [1, 2, 3, 4];
        let relocations = 0u32.to_le_bytes();
        let isa = DescriptorWords::arm(fae1::registry::isa::arm::THUMB_2);
        let common = |abi| {
            concatenate_and_pad_bytearray_for_format(
                &startup,
                symbols(),
                &relocations,
                4,
                32,
                &[0xAA; 24],
                OutputFormat::Fae1 { isa, abi },
            )
            .unwrap()
        };
        let alias = common(Fae1Abi::Rustlet {
            kind: RustletProfileKind::Application,
            minimum_version: AbiVersion {
                major: 1,
                minor: 0,
                patch: 0,
                revision: 0,
            },
            stack_size: 2048,
        });
        let explicit = common(Fae1Abi::Explicit(
            DescriptorWords::new(0xAC1D_A992, &[0xA990_1000, 0x0001_0040]).unwrap(),
        ));

        assert_eq!(alias, explicit);
    }
}
