use std::ffi::CString;
use std::io::{self, Write};
use byteorder::{BigEndian, WriteBytesExt};

use {ErrorKind, Result};
use isobmff::{BoxHeader, BoxType, Brand, FullBoxHeader, HandlerType, SampleEntry};

// macro_rules! write_u8 {
//     ($w:expr, $n:expr) => { track_io!($w.write_u8($n))?; }
// }
macro_rules! write_u16 {
    ($w:expr, $n:expr) => { track_io!($w.write_u16::<BigEndian>($n))?; }
}
macro_rules! write_i16 {
    ($w:expr, $n:expr) => { track_io!($w.write_i16::<BigEndian>($n))?; }
}
macro_rules! write_u32 {
    ($w:expr, $n:expr) => { track_io!($w.write_u32::<BigEndian>($n))?; }
}
macro_rules! write_i32 {
    ($w:expr, $n:expr) => { track_io!($w.write_i32::<BigEndian>($n))?; }
}
// macro_rules! write_u64 {
//     ($w:expr, $n:expr) => { track_io!($w.write_u64::<BigEndian>($n))?; }
// }
macro_rules! write_all {
    ($w:expr, $n:expr) => { track_io!($w.write_all($n))?; }
}
macro_rules! write_zeroes {
    ($w:expr, $n:expr) => { track_io!($w.write_all(&[0;$n][..]))?; }
}
macro_rules! write_box {
    ($w:expr, $b:expr) => { track!($b.write_box_to(&mut $w))?; }
}
macro_rules! write_boxes {
    ($w:expr, $bs:expr) => {
        for b in $bs {
            track!(b.write_box_to(&mut $w))?;
        }
    }
}

#[derive(Debug)]
pub struct WriteBytesCounter(u64);
impl WriteBytesCounter {
    pub fn new() -> Self {
        WriteBytesCounter(0)
    }
    pub fn count(&self) -> u64 {
        self.0
    }
}
impl Write for WriteBytesCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub trait WriteTo {
    fn write_to<W: Write>(&self, writer: W) -> Result<()>;
}

pub trait WriteBoxTo: WriteTo {
    fn box_type(&self) -> BoxType;
    fn box_size(&self) -> u32 {
        let mut writer = WriteBytesCounter::new();
        track_try_unwrap!(self.write_to(&mut writer));

        let mut size = 8 + writer.count() as u32;
        if self.full_box_header().is_some() {
            size += 4;
        }
        size
    }
    fn box_header(&self) -> BoxHeader {
        BoxHeader {
            kind: self.box_type(),
            size: self.box_size(),
        }
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        None
    }
    fn write_box_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track!(self.box_header().write_to(&mut writer))?;
        if let Some(x) = self.full_box_header() {
            track!(x.write_to(&mut writer))?;
        }
        track!(self.write_to(writer))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct File {
    pub ftyp_box: FileTypeBox,
    pub moov_box: MovieBox,
    // TODO:
    // pub mdat_boxes: Vec<MediaDataBox>,
    // pub moof_boxes: Vec<MoofBox>,
    pub mfra_box: MovieFragmentRandomAccessBox,
}
impl File {
    pub fn new() -> File {
        File {
            ftyp_box: FileTypeBox::default(),
            moov_box: MovieBox::new(),
            mfra_box: MovieFragmentRandomAccessBox::new(),
        }
    }
}
impl WriteTo for File {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.ftyp_box);
        write_box!(writer, self.moov_box);
        write_box!(writer, self.mfra_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieFragmentRandomAccessBox {
    pub mfro_box: MovieFragmentRandomAccessOffsetBox,
    // TOOD(?): tfra_boxes
}
impl MovieFragmentRandomAccessBox {
    pub fn new() -> Self {
        MovieFragmentRandomAccessBox {
            mfro_box: MovieFragmentRandomAccessOffsetBox::new(),
        }
    }
}
impl WriteBoxTo for MovieFragmentRandomAccessBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mfra")
    }
    fn box_size(&self) -> u32 {
        let mut writer = WriteBytesCounter::new();
        track_try_unwrap!(self.mfro_box.write_box_to(&mut writer));
        8 + writer.count() as u32
    }
}
impl WriteTo for MovieFragmentRandomAccessBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        let mut mfro_box = self.mfro_box.clone();
        mfro_box.size = self.box_size();
        write_box!(writer, mfro_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieBox {
    pub mvhd_box: MovieHeaderBox,
    pub trak_boxes: Vec<TrackBox>,
    pub mvex_box: MovieExtendsBox,
}
impl MovieBox {
    pub fn new() -> Self {
        MovieBox {
            mvhd_box: MovieHeaderBox::new(),
            trak_boxes: Vec::new(),
            mvex_box: MovieExtendsBox::new(),
        }
    }
}
impl WriteBoxTo for MovieBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"moov")
    }
}
impl WriteTo for MovieBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_assert!(!self.trak_boxes.is_empty(), ErrorKind::InvalidInput);

        write_box!(writer, self.mvhd_box);
        write_boxes!(writer, &self.trak_boxes);
        write_box!(writer, self.mvex_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieExtendsBox {
    pub mehd_box: MovieExtendsHeaderBox,
    pub trex_boxes: Vec<TrackExtendsBox>,
}
impl MovieExtendsBox {
    pub fn new() -> Self {
        MovieExtendsBox {
            mehd_box: MovieExtendsHeaderBox::new(),
            trex_boxes: Vec::new(), // FIXME
        }
    }
}
impl WriteBoxTo for MovieExtendsBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mvex")
    }
}
impl WriteTo for MovieExtendsBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mehd_box);
        write_boxes!(writer, &self.trex_boxes);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackBox {
    pub tkhd_box: TrackHeaderBox,
    pub mdia_box: MediaBox,
}
impl TrackBox {
    pub fn new(is_video: bool) -> Self {
        TrackBox {
            tkhd_box: TrackHeaderBox::new(is_video),
            mdia_box: MediaBox::new(is_video),
        }
    }
}
impl WriteBoxTo for TrackBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"trak")
    }
}
impl WriteTo for TrackBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.tkhd_box);
        write_box!(writer, self.mdia_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaBox {
    pub mdhd_box: MediaHeaderBox,
    pub hdlr_box: HandlerReferenceBox,
    pub minf_box: MediaInformationBox,
}
impl MediaBox {
    pub fn new(is_video: bool) -> Self {
        MediaBox {
            mdhd_box: MediaHeaderBox::new(),
            hdlr_box: HandlerReferenceBox::new(is_video),
            minf_box: MediaInformationBox::new(is_video),
        }
    }
}
impl WriteBoxTo for MediaBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mdia")
    }
}
impl WriteTo for MediaBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.mdhd_box);
        write_box!(writer, self.hdlr_box);
        write_box!(writer, self.minf_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaInformationBox {
    pub vmhd_box: Option<VideoMediaHeaderBox>,
    pub smhd_box: Option<SoundMediaHeaderBox>,
    pub dinf_box: DataInformationBox,
    pub stbl_box: SampleTableBox,
}
impl MediaInformationBox {
    pub fn new(is_video: bool) -> Self {
        MediaInformationBox {
            vmhd_box: if is_video {
                Some(VideoMediaHeaderBox::new())
            } else {
                None
            },
            smhd_box: if !is_video {
                Some(SoundMediaHeaderBox::new())
            } else {
                None
            },
            dinf_box: DataInformationBox::new(),
            stbl_box: SampleTableBox::new(),
        }
    }
}
impl WriteBoxTo for MediaInformationBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"minf")
    }
}
impl WriteTo for MediaInformationBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        if let Some(ref x) = self.vmhd_box {
            write_box!(writer, x);
        }
        if let Some(ref x) = self.smhd_box {
            write_box!(writer, x);
        }
        write_box!(writer, self.dinf_box);
        write_box!(writer, self.stbl_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleTableBox {
    pub stsd_box: SampleDescriptionBox,
    pub stts_box: TimeToSampleBox,
    pub stsc_box: SampleToChunkBox,
    pub stsz_box: SampleSizeBox,
    pub stco_box: ChunkOffsetBox,
}
impl SampleTableBox {
    pub fn new() -> Self {
        SampleTableBox {
            stsd_box: SampleDescriptionBox::new(),
            stts_box: TimeToSampleBox,
            stsc_box: SampleToChunkBox,
            stsz_box: SampleSizeBox,
            stco_box: ChunkOffsetBox,
        }
    }
}
impl WriteBoxTo for SampleTableBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stbl")
    }
}
impl WriteTo for SampleTableBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.stsd_box);
        write_box!(writer, self.stts_box);
        write_box!(writer, self.stsc_box);
        write_box!(writer, self.stsz_box);
        write_box!(writer, self.stco_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleSizeBox;
impl WriteBoxTo for SampleSizeBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stsz")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SampleSizeBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TimeToSampleBox;
impl WriteBoxTo for TimeToSampleBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stts")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for TimeToSampleBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ChunkOffsetBox;
impl WriteBoxTo for ChunkOffsetBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stco")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for ChunkOffsetBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleToChunkBox;
impl WriteBoxTo for SampleToChunkBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stsc")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SampleToChunkBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SampleDescriptionBox {
    pub sample_entries: Vec<SampleEntry>,
}
impl SampleDescriptionBox {
    pub fn new() -> Self {
        SampleDescriptionBox {
            sample_entries: Vec::new(), // FIXME
        }
    }
}
impl WriteBoxTo for SampleDescriptionBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"stsd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SampleDescriptionBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.sample_entries.len() as u32);
        write_boxes!(writer, &self.sample_entries);
        Ok(())
    }
}
impl WriteBoxTo for SampleEntry {
    fn box_type(&self) -> BoxType {
        BoxType(self.format.0)
    }
}
impl WriteTo for SampleEntry {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 6);
        write_u16!(writer, self.data_reference_index);
        write_all!(writer, &self.data);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MovieFragmentRandomAccessOffsetBox {
    pub size: u32,
}
impl MovieFragmentRandomAccessOffsetBox {
    pub fn new() -> Self {
        MovieFragmentRandomAccessOffsetBox{
            size: 0 // NOTE
        }
    }
}
impl WriteBoxTo for MovieFragmentRandomAccessOffsetBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mfro")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for MovieFragmentRandomAccessOffsetBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.size);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataInformationBox {
    pub dref_box: DataReferenceBox,
}
impl DataInformationBox {
    pub fn new() -> Self {
        DataInformationBox {
            dref_box: DataReferenceBox::new(),
        }
    }
}
impl WriteBoxTo for DataInformationBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"dinf")
    }
}
impl WriteTo for DataInformationBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_box!(writer, self.dref_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataReferenceBox {
    pub url_box: DataEntryUrlBox,
}
impl DataReferenceBox {
    pub fn new() -> Self {
        DataReferenceBox {
            url_box: DataEntryUrlBox::new(),
        }
    }
}
impl WriteBoxTo for DataReferenceBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"dref")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for DataReferenceBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, 1);
        write_box!(writer, self.url_box);
        Ok(())
    }
}

#[derive(Debug)]
pub struct DataEntryUrlBox {
    pub location: Option<CString>,
}
impl DataEntryUrlBox {
    pub fn new() -> Self {
        DataEntryUrlBox { location: None }
    }
}
impl WriteBoxTo for DataEntryUrlBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"url ")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        let flags = if self.location.is_some() {
            0
        } else {
            0x00_0001
        };
        Some(FullBoxHeader::new(0, flags))
    }
}
impl WriteTo for DataEntryUrlBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        if let Some(ref x) = self.location {
            write_all!(writer, x.as_bytes_with_nul());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct VideoMediaHeaderBox {
    pub graphicsmode: u16,
    pub opcolor: [u16; 3],
}
impl VideoMediaHeaderBox {
    pub fn new() -> Self {
        VideoMediaHeaderBox {
            graphicsmode: 0,
            opcolor: [0, 0, 0],
        }
    }
}
impl WriteBoxTo for VideoMediaHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"vmhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 1))
    }
}
impl WriteTo for VideoMediaHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u16!(writer, self.graphicsmode);
        for &x in &self.opcolor {
            write_u16!(writer, x);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SoundMediaHeaderBox {
    pub balance: i16,
}
impl SoundMediaHeaderBox {
    pub fn new() -> Self {
        SoundMediaHeaderBox { balance: 0 }
    }
}
impl WriteBoxTo for SoundMediaHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"smhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for SoundMediaHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_i16!(writer, self.balance);
        write_zeroes!(writer, 2);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MediaHeaderBox {
    pub creation_time: u32,
    pub modification_time: u32,
    pub timescale: u32,
    pub duration: u32,
    pub language: u16,
}
impl MediaHeaderBox {
    pub fn new() -> Self {
        MediaHeaderBox {
            creation_time: 0,
            modification_time: 0,
            timescale: 0, // FIXME
            duration: 1,  // FIXME
            language: 21956,
        }
    }
}
impl WriteBoxTo for MediaHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mdhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for MediaHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.creation_time);
        write_u32!(writer, self.modification_time);
        write_u32!(writer, self.timescale);
        write_u32!(writer, self.duration);
        write_u16!(writer, self.language);
        write_zeroes!(writer, 2);
        Ok(())
    }
}

#[derive(Debug)]
pub struct HandlerReferenceBox {
    pub handler_type: HandlerType,
    pub name: CString,
}
impl HandlerReferenceBox {
    pub fn new(is_video: bool) -> Self {
        HandlerReferenceBox {
            handler_type: HandlerType(if is_video { *b"vide" } else { *b"soun" }),
            name: CString::new("A handler").expect("Never fails"),
        }
    }
}
impl WriteBoxTo for HandlerReferenceBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"hdlr")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for HandlerReferenceBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_zeroes!(writer, 4);
        write_all!(writer, &self.handler_type.0);
        write_zeroes!(writer, 4 * 3);
        write_all!(writer, self.name.as_bytes_with_nul());
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackHeaderBox {
    pub track_enabled: bool,
    pub track_in_movie: bool,
    pub track_in_preview: bool,
    pub track_size_is_aspect_ratio: bool,
    pub creation_time: u32,
    pub modification_time: u32,
    pub track_id: u32,
    pub duration: u32,
    pub layer: i16,
    pub alternate_group: i16,
    pub volume: i16, // fixed point 8.8
    pub matrix: [i32; 9],
    pub width: u32,  // fixed point 16.16
    pub height: u32, // fixed point 16.16
}
impl TrackHeaderBox {
    pub fn new(is_video: bool) -> Self {
        TrackHeaderBox {
            track_enabled: true,
            track_in_movie: true,
            track_in_preview: true,
            track_size_is_aspect_ratio: false,
            creation_time: 0,
            modification_time: 0,
            track_id: if is_video { 1 } else { 2 },
            duration: 1, // FIXME
            layer: 0,
            alternate_group: 0,
            volume: if is_video { 0 } else { 256 },
            matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
            width: 0,
            height: 0,
        }
    }
}
impl WriteBoxTo for TrackHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"tkhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        let flags = (self.track_enabled as u32 * 0x00_0001)
            | (self.track_in_movie as u32 * 0x00_0002)
            | (self.track_in_preview as u32 * 0x00_0004)
            | (self.track_size_is_aspect_ratio as u32 * 0x00_0008);
        Some(FullBoxHeader::new(0, flags))
    }
}
impl WriteTo for TrackHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.creation_time);
        write_u32!(writer, self.modification_time);
        write_u32!(writer, self.track_id);
        write_zeroes!(writer, 4);
        write_u32!(writer, self.duration);
        write_zeroes!(writer, 4 * 2);
        write_i16!(writer, self.layer);
        write_i16!(writer, self.alternate_group);
        write_i16!(writer, self.volume);
        write_zeroes!(writer, 2);
        for &x in &self.matrix {
            write_i32!(writer, x);
        }
        write_u32!(writer, self.width);
        write_u32!(writer, self.height);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieHeaderBox {
    pub creation_time: u32,
    pub modification_time: u32,
    pub timescale: u32,
    pub duration: u32,
    pub rate: i32,   // fixed point 16.16
    pub volume: i16, // fixed point 8.8
    pub matrix: [i32; 9],
    pub next_track_id: u32, // 0xFFFF_FFFF means ...
}
impl MovieHeaderBox {
    pub fn new() -> Self {
        MovieHeaderBox {
            creation_time: 0,
            modification_time: 0,
            timescale: 1, // FIXME
            duration: 1,  // FIXME
            rate: 65536,
            volume: 256,
            matrix: [65536, 0, 0, 0, 65536, 0, 0, 0, 1073741824],
            next_track_id: 0xFFFF_FFFF,
        }
    }
}
impl WriteBoxTo for MovieHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mvhd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for MovieHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.creation_time);
        write_u32!(writer, self.modification_time);
        write_u32!(writer, self.timescale);
        write_u32!(writer, self.duration);
        write_i32!(writer, self.rate);
        write_i16!(writer, self.volume);
        write_zeroes!(writer, 2);
        write_zeroes!(writer, 4 * 2);
        for &x in &self.matrix {
            write_i32!(writer, x);
        }
        write_zeroes!(writer, 4 * 6);
        write_u32!(writer, self.next_track_id);
        Ok(())
    }
}

#[derive(Debug)]
pub struct MovieExtendsHeaderBox {
    pub fragment_duration: u32,
}
impl MovieExtendsHeaderBox {
    pub fn new() -> Self {
        MovieExtendsHeaderBox {
            fragment_duration: 1, // FIXME
        }
    }
}
impl WriteBoxTo for MovieExtendsHeaderBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"mehd")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for MovieExtendsHeaderBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.fragment_duration);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TrackExtendsBox {
    pub track_id: u32,
    pub default_sample_description_index: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}
impl TrackExtendsBox {
    pub fn new(track_id: u32) -> Self {
        TrackExtendsBox {
            track_id,
            default_sample_description_index: 1,
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}
impl WriteBoxTo for TrackExtendsBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"trex")
    }
    fn full_box_header(&self) -> Option<FullBoxHeader> {
        Some(FullBoxHeader::new(0, 0))
    }
}
impl WriteTo for TrackExtendsBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_u32!(writer, self.track_id);
        write_u32!(writer, self.default_sample_description_index);
        write_u32!(writer, self.default_sample_duration);
        write_u32!(writer, self.default_sample_size);
        write_u32!(writer, self.default_sample_flags);
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileTypeBox {
    pub major_brand: Brand,
    pub minor_version: u32,
    pub compatible_brands: Vec<Brand>,
}
impl WriteBoxTo for FileTypeBox {
    fn box_type(&self) -> BoxType {
        BoxType(*b"ftyp")
    }
}
impl WriteTo for FileTypeBox {
    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        write_all!(writer, &self.major_brand.0);
        write_u32!(writer, self.minor_version);
        for brand in &self.compatible_brands {
            write_all!(writer, &brand.0);
        }
        Ok(())
    }
}
impl Default for FileTypeBox {
    fn default() -> Self {
        FileTypeBox {
            major_brand: Brand(*b"isom"),
            minor_version: 512,
            compatible_brands: Vec::new(),
        }
    }
}