pub mod common {
    pub use fae_core::common::*;
}
pub mod constants {
    pub use fae_core::constants::*;
}
pub mod fae_format {
    pub use fae_core::fae_format::*;
}
pub use build_gdbinit::gdbinit;
pub use fae_build::build_tool;
pub use fae_elf::elf;
pub use fae_elf::rust_input_section_plan;
pub use fae_read::read_tool;

#[cfg(test)]
mod tests {
    use fae_core::common::{get_bytes_text_suffix, get_word_from_slice, round_up_pow2, to_word};
    use fae_core::constants;
    use fae_core::fae_format::{
        compute_aligned_header_size, compute_header_size, compute_text_alignment,
        concatenate_and_pad_bytearray, padded_exported_symbols, ExportedSymbols,
    };

    #[test]
    fn to_word_and_get_word_roundtrip() {
        let w = 0x1234_5678u32;
        let bytes = to_word(w);
        assert_eq!(bytes, [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(get_word_from_slice(&bytes, 0).unwrap(), w);
    }

    #[test]
    fn get_word_from_slice_supports_negative_offsets() {
        let data = [0xaa, 0xbb, 0xcc, 0xdd, 0x78, 0x56, 0x34, 0x12];
        assert_eq!(get_word_from_slice(&data, -4).unwrap(), 0x1234_5678);
    }

    #[test]
    fn get_word_from_slice_rejects_out_of_bounds_reads() {
        let err = get_word_from_slice(&[1, 2, 3], 0).unwrap_err().to_string();
        assert!(err.contains("out-of-bounds word read"));
    }

    #[test]
    fn round_up_pow2_rounds_to_next_multiple() {
        assert_eq!(round_up_pow2(0, 32), 0);
        assert_eq!(round_up_pow2(1, 32), 32);
        assert_eq!(round_up_pow2(32, 32), 32);
        assert_eq!(round_up_pow2(33, 32), 64);
    }

    #[test]
    fn get_bytes_text_suffix_handles_singular_and_plural() {
        assert_eq!(get_bytes_text_suffix(0), "byte");
        assert_eq!(get_bytes_text_suffix(1), "byte");
        assert_eq!(get_bytes_text_suffix(2), "bytes");
    }

    #[test]
    fn compute_header_size_and_alignment_follow_current_format() {
        let crt0 = vec![0xaa; 10];
        let reloc = vec![0x00; 12];

        assert_eq!(compute_header_size(&crt0, &reloc), 26);
        assert_eq!(compute_text_alignment(&crt0, &reloc, 0), 0);
        assert_eq!(compute_text_alignment(&crt0, &reloc, 32), 2);
        assert_eq!(compute_text_alignment(&crt0, &reloc, 64), 34);
        assert_eq!(compute_aligned_header_size(&crt0, &reloc, 32), 32);
        assert_eq!(compute_aligned_header_size(&crt0, &reloc, 64), 64);
    }

    #[test]
    fn concatenate_and_pad_bytearray_writes_expected_footer_and_partition() {
        let crt0 = vec![0xaa; 10];
        let reloc = vec![0x02, 0x00, 0x00, 0x00];
        let part = vec![0x11; 23];
        let raw_symbols = ExportedSymbols {
            start: 7,
            rom_ram_size: 3,
            rom_size: 9,
            got_size: 5,
            ram_size: 4,
        };
        let symbols = padded_exported_symbols(raw_symbols);
        let out = concatenate_and_pad_bytearray(
            &crt0,
            symbols,
            &reloc,
            32,
            constants::PADDING_MPU_ALIGNMENT,
            &part,
        );

        assert_eq!(out.len() % constants::PADDING_MPU_ALIGNMENT, 0);
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_MAGIC_NUMBER_AND_VERSION_OFFSET).unwrap(),
            constants::MAGIC_NUMBER_AND_VERSION
        );
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_CRT0_OFFSET).unwrap(),
            crt0.len() as u32
        );
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_ENTRYPOINT_OFFSET).unwrap(),
            symbols.start
        );
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_ROM_RAM_SIZE_OFFSET).unwrap(),
            symbols.rom_ram_size
        );
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_ROM_SIZE_OFFSET).unwrap(),
            symbols.rom_size
        );
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_GOT_SIZE_OFFSET).unwrap(),
            symbols.got_size
        );
        assert_eq!(
            get_word_from_slice(&out, constants::FOOTER_RAM_SIZE_OFFSET).unwrap(),
            symbols.ram_size
        );

        let payload_padding_offset = compute_header_size(&crt0, &reloc);
        assert_eq!(
            get_word_from_slice(&out, payload_padding_offset as isize).unwrap(),
            compute_text_alignment(&crt0, &reloc, 32) as u32
        );
        let partition_offset = compute_aligned_header_size(&crt0, &reloc, 32);
        assert_eq!(
            &out[partition_offset..partition_offset + part.len()],
            part.as_slice()
        );
        assert_eq!(partition_offset % 32, 0);
    }

    #[test]
    fn concatenate_and_pad_bytearray_honors_final_size_alignment() {
        let crt0 = vec![0xaa; 10];
        let reloc = vec![0x00; 4];
        let part = vec![0x11; 23];
        let symbols = ExportedSymbols {
            start: 0,
            rom_ram_size: 0,
            rom_size: 4,
            got_size: 0,
            ram_size: 0,
        };

        let out = concatenate_and_pad_bytearray(&crt0, symbols, &reloc, 0, 128, &part);

        assert_eq!(out.len() % 128, 0);
    }
}
