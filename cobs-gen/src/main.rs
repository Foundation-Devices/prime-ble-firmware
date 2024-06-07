
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum MsgKind {
    BtData = 0x01,
    SystemStatus,
    FwUpdate,
    BtDeviceNearby,
}


#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub msg_type : MsgKind,
    pub msg  :  [u8; 16],
}

fn main() {
    use postcard::to_slice_cobs;

    // Buffer for cobs
    let mut buf = [0u8; 64];
    // Set some byte here to get the desided cob
    let mut message = [0u8; 16];
    //message[0]=2;

    let used = to_slice_cobs(&Message { msg_type: MsgKind::SystemStatus, msg: message }, &mut buf).unwrap();
    println!("{:02X?}",used);
}
