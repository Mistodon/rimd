use std::error;
use std::io::{IoError,Reader};
use std::num::FromPrimitive;
use std::fmt;

/// An error that can occur trying to parse a midi message
pub enum MidiError {
    InvalidStatus(u8),
    OtherErr(&'static str),
    IoError(IoError),
}

impl error::FromError<IoError> for MidiError {
    fn from_error(err: IoError) -> MidiError {
        MidiError::IoError(err)
    }
}

impl error::Error for MidiError {
    fn description(&self) -> &str {
        match *self {
            MidiError::InvalidStatus(_) => "Midi data has invalid status byte",
            MidiError::OtherErr(_) => "A general midi error has occured",
            MidiError::IoError(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            MidiError::IoError(ref err) => Some(err as &error::Error),
            _ => None,
        }
    }
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MidiError::InvalidStatus(ref s) => write!(f,"Invalid Midi status: {}",s),
            MidiError::OtherErr(ref s) => write!(f,"Midi Error: {}",s),
            MidiError::IoError(ref e) => write!(f,"{}",e),
        }
    }
}

/// The status field of a midi message indicates what midi command it
/// represents and what channel it is on
#[derive(FromPrimitive)]
pub enum Status {
    // voice
    NoteOff = 0x80,
    NoteOn = 0x90,
    PolyphonicAftertouch = 0xA0,
    ControlChange = 0xB0,
    ProgramChange = 0xC0,
    ChannelAftertouch = 0xD0,
    PitchBend = 0xE0,

    // sysex
    SysExStart = 0xF0,
    MIDITimeCodeQtrFrame = 0xF1,
    SongPositionPointer = 0xF2,
    SongSelect = 0xF3,
    TuneRequest = 0xF6, // F4 anf 5 are reserved and unused
    SysExEnd = 0xF7,
    TimingClock = 0xF8,
    Start = 0xFA,
    Continue = 0xFB,
    Stop = 0xFC,
    ActiveSensing = 0xFE, // FD also res/unused
    SystemReset = 0xFF,
}


/// Midi message building and parsing.  See
/// http://www.midi.org/techspecs/midimessages.php for a description
/// of the various Midi messages that exist.
pub struct MidiMessage {
    data: Vec<u8>,
}

static STATUS_MASK: u8 = 0xF0;
static CHANNEL_MASK: u8 = 0x0F;

impl MidiMessage {
    /// Return the status (type) of this message
    pub fn status(&self) -> Status {
        FromPrimitive::from_u8(self.data[0] & STATUS_MASK).unwrap()
    }

    /// Return the channel this message is on (TODO: return 0 for messages with no channel)
    pub fn channel(&self) -> u8 {
        (self.data[0] & CHANNEL_MASK) + 1
    }

    /// Get te data at index `index` from this message.  Status is at
    /// index 0
    pub fn data(&self, index: usize) -> u8 {
        self.data[index]
    }

    // Or in the channel bits to a status
    fn make_status(status: Status, channel: u8) -> u8 {
        status as u8 | channel
    }

    /// Create a midi message from a vector of bytes
    pub fn from_bytes(bytes: Vec<u8>) -> MidiMessage{
        // TODO: Validate bytes
        MidiMessage {
            data: bytes,
        }
    }

    // return the number of data bytes for a message with the given status
    // -1 -> variable sized message, call get_variable_size
    // -2 -> sysex, read until SysExEnd
    // -3 -> invalid status
    fn data_bytes(status: u8) -> isize {
        match FromPrimitive::from_u8(status & STATUS_MASK) {
            Some(stat) => {
                match stat {
                    Status::NoteOff |
                    Status::NoteOn |
                    Status::PolyphonicAftertouch |
                    Status::ControlChange |
                    Status::PitchBend |
                    Status::SongPositionPointer => { 2 }

                    Status::SysExStart => { -2 }

                    Status::ProgramChange |
                    Status::ChannelAftertouch |
                    Status::MIDITimeCodeQtrFrame |
                    Status::SongSelect => { 1 }

                    Status::TuneRequest |
                    Status::SysExEnd |
                    Status::TimingClock |
                    Status::Start |
                    Status::Continue |
                    Status::Stop |
                    Status::ActiveSensing |
                    Status::SystemReset => { 0 }
                }
            }
            None => -3
        }
    }

    /// Get the next midi message from the reader given that the
    /// status `stat` has just been read
    pub fn next_message_given_status(stat: u8, reader: &mut Reader) -> Result<MidiMessage, MidiError> {
        let mut ret:Vec<u8> = Vec::with_capacity(3);
        ret.push(stat);
        match MidiMessage::data_bytes(stat) {
            0 => {}
            1 => { ret.push(try!(reader.read_byte())); }
            2 => { ret.push(try!(reader.read_byte()));
                   ret.push(try!(reader.read_byte())); }
            -1 => { return Err(MidiError::OtherErr("Don't handle variable sized yet")); }
            -2 => { return Err(MidiError::OtherErr("Don't handle sysex yet")); }
            _ =>  { return Err(MidiError::InvalidStatus(stat)); }
        }
        Ok(MidiMessage{data: ret})
    }

    /// Extract next midi message from a reader
    pub fn next_message(reader: &mut Reader) -> Result<MidiMessage,MidiError> {
        let stat = try!(reader.read_byte());
        MidiMessage::next_message_given_status(stat,reader)
    }


    // Functions to build midi messages

    /// Create a note on message
    pub fn note_on(note: u8, velocity: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::NoteOn,channel), note, velocity],
        }
    }

    /// Create a note off message
    pub fn note_off(note: u8, velocity: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::NoteOff,channel), note, velocity],
        }
    }

    /// Create a polyphonic aftertouch message
    /// This message is most often sent by pressing down on the key after it "bottoms out".
    pub fn polyphonic_aftertouch(note: u8, pressure: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::PolyphonicAftertouch,channel), note, pressure],
        }
    }

    /// Create a control change message
    /// This message is sent when a controller value changes. Controllers include devices such as
    /// pedals and levers. Controller numbers 120-127 are reserved as "Channel Mode Messages".
    pub fn control_change(controler: u8, data: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::ControlChange,channel), controler, data],
        }
    }

    /// Create a program change message
    /// This message sent when the patch number changes. `program` is the new program number.
    pub fn program_change(program: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::ProgramChange,channel), program],
        }
    }

    /// Create a channel aftertouch
    /// This message is most often sent by pressing down on the key after it "bottoms out". This message
    /// is different from polyphonic after-touch. Use this message to send the single greatest pressure
    /// value (of all the current depressed keys). `pressure` is the pressure value.
    pub fn channel_aftertouch(pressure: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::ChannelAftertouch,channel), pressure],
        }
    }

    /// Create a pitch bench message
    /// This message is sent to indicate a change in the pitch bender (wheel or lever, typically).
    /// The pitch bender is measured by a fourteen bit value. Center (no pitch change) is 2000H.
    /// Sensitivity is a function of the transmitter. `lsb` are the least significant 7 bits.
    /// `msb` are the most significant 7 bits.
    pub fn pitch_bend(lsb: u8, msb: u8, channel: u8) -> MidiMessage {
        MidiMessage {
            data: vec![MidiMessage::make_status(Status::PitchBend,channel), lsb, msb],
        }
    }

}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}",
               match *self {
                   Status::NoteOff => "Note Off",
                   Status::NoteOn => "Note On",
                   Status::PolyphonicAftertouch => "Polyphonic Aftertouch",
                   Status::ControlChange => "Control Change",
                   Status::ProgramChange => "Program Change",
                   Status::ChannelAftertouch => "Channel Aftertouch",
                   Status::PitchBend => "Pitch Bend",
                   Status::SysExStart => "SysEx Start",
                   Status::MIDITimeCodeQtrFrame => "MIDI Time Code Qtr Frame",
                   Status::SongPositionPointer => "Song Position Pointer",
                   Status::SongSelect => "Song Select",
                   Status::TuneRequest => "Tune Request",
                   Status::SysExEnd => "SysEx End",
                   Status::TimingClock => "Timing Clock",
                   Status::Start => "Start",
                   Status::Continue => "Continue",
                   Status::Stop => "Stop",
                   Status::ActiveSensing => "Active Sensing",
                   Status::SystemReset => "System Reset",
               })
    }
}

impl fmt::Display for MidiMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.data.len() == 2 {
            write!(f, "{}: [{}]\tchannel: {}", self.status(),self.data[1],self.channel())
        }
        else if self.data.len() == 3 {
            write!(f, "{}: [{},{}]\tchannel: {}", self.status(),self.data[1],self.data[2],self.channel())
        }
        else {
            write!(f, "{}: [no data]\tchannel: {}", self.status(),self.channel())
        }
    }
}
