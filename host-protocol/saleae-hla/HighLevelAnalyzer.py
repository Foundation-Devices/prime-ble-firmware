# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

"""Saleae Logic 2 High Level Analyzer for host-protocol messages.

Decodes postcard-encoded HostProtocolMessage frames exchanged over SPI
between the host MPU and the nRF52805 BLE controller.

SPI transaction flow:
  Transaction N:   MOSI = postcard-encoded request, MISO = zeros
  Transaction N+1: MOSI = don't-care, MISO = 2-byte BE length prefix + postcard-encoded response

Install: In Saleae Logic 2 -> Extensions -> Load Existing Extension -> select this directory.
Stack on top of an SPI analyzer with chip-select enabled.
"""

from saleae.analyzers import HighLevelAnalyzer, AnalyzerFrame


# ---------------------------------------------------------------------------
# Postcard primitive decoders
# ---------------------------------------------------------------------------

def read_varint(data, pos):
    """Decode an unsigned LEB128 varint. Returns (value, new_pos)."""
    result = 0
    shift = 0
    while pos < len(data):
        byte = data[pos]
        pos += 1
        result |= (byte & 0x7F) << shift
        if not (byte & 0x80):
            return result, pos
        shift += 7
        if shift >= 70:
            raise ValueError("varint overflow")
    raise ValueError("truncated varint")


def read_u8(data, pos):
    if pos >= len(data):
        raise ValueError("truncated u8")
    return data[pos], pos + 1


def read_i8(data, pos):
    """Read a raw i8 (two's complement byte)."""
    val, pos = read_u8(data, pos)
    return val - 256 if val >= 128 else val, pos


def read_bool(data, pos):
    val, pos = read_u8(data, pos)
    return bool(val), pos


def read_bytes(data, pos, n):
    if pos + n > len(data):
        raise ValueError("truncated fixed bytes")
    return data[pos:pos + n], pos + n


def read_string(data, pos):
    """Read a postcard string: varint length + UTF-8 bytes."""
    length, pos = read_varint(data, pos)
    if pos + length > len(data):
        raise ValueError("truncated string")
    return data[pos:pos + length].decode("utf-8", errors="replace"), pos + length


def read_vec_len(data, pos):
    """Read the varint length prefix of a Vec / &[u8]."""
    return read_varint(data, pos)


# ---------------------------------------------------------------------------
# Lookup tables
# ---------------------------------------------------------------------------

TX_POWER = {
    0: "-40dBm", 1: "-20dBm", 2: "-16dBm", 3: "-12dBm",
    4: "-8dBm", 5: "-4dBm", 6: "0dBm", 7: "+3dBm", 8: "+4dBm",
}

STATE = {0: "Enabled", 1: "Disabled", 2: "FirmwareUpgrade", 3: "Unknown"}

CONNECTION_STATUS = {0: "Disabled", 1: "WaitingForConnection", 2: "Connected"}

SEND_DATA_RESPONSE = {0: "Sent", 1: "BufferFull"}

SECRET_SAVE_RESPONSE = {0: "NotAllowed", 1: "Sealed", 2: "Error"}

TRUST_LEVEL = {0: "Full", 1: "Developer"}

POSTCARD_ERROR = {0: "Deser", 1: "OverFull"}

ADV_CHAN_BITS = {5: "C37", 6: "C38", 7: "C39"}

# First MISO byte during a request transaction identifies the active firmware.
MISO_TARGET = {0x69: "Bootloader", 0x51: "Application"}


# ---------------------------------------------------------------------------
# Sub-message decoders
# ---------------------------------------------------------------------------

# Bluetooth variants with no payload — discriminant -> name
_BT_SIMPLE = {
    1: "AckDisableChannels", 2: "NackDisableChannels",
    3: "Enable", 4: "AckEnable", 5: "Disable", 6: "AckDisable",
    7: "GetStatus", 11: "GetReceivedData", 13: "NoReceivedData",
    14: "GetFirmwareVersion", 16: "GetBtAddress", 19: "AckTxPower",
    20: "GetDeviceId", 22: "Disconnect", 23: "AckDisconnect",
    25: "AckSetDeviceName",
}

# Bootloader variants with no payload — discriminant -> name
_BL_SIMPLE = {
    0: "EraseFirmware", 1: "AckEraseFirmware",
    2: "NackEraseFirmwareRead", 3: "NackEraseFirmware",
    4: "NackEraseFirmwareWrite", 11: "NoCosignHeader",
    12: "FirmwareVersion", 14: "BootloaderVersion",
}


def _fmt_adv_chan(byte):
    parts = [name for bit, name in ADV_CHAN_BITS.items() if byte & (1 << bit)]
    return " | ".join(parts) if parts else f"0x{byte:02X}"


def decode_bluetooth(data, pos):
    """Decode a Bluetooth sub-message starting at *pos* (after top-level discriminant 0)."""
    sub, pos = read_varint(data, pos)

    if sub in _BT_SIMPLE:
        return f"BT::{_BT_SIMPLE[sub]}"

    if sub == 0:  # DisableChannels(AdvChan)
        chan, pos = read_u8(data, pos)
        return f"BT::DisableChannels({_fmt_adv_chan(chan)})"

    if sub == 8:  # Status(BluetoothStatus)
        conn, pos = read_varint(data, pos)
        # queue_overflow was added later; treat it as optional for backward compat.
        if conn == 2:  # Connected
            rssi, pos = read_i8(data, pos)
            extra = ""
            if pos < len(data):
                overflow, pos = read_bool(data, pos)
                if overflow:
                    extra = ", overflow"
            return f"BT::Status(Connected, rssi={rssi}{extra})"
        conn_name = CONNECTION_STATUS.get(conn, f"?{conn}")
        extra = ""
        if pos < len(data):
            overflow, pos = read_bool(data, pos)
            if overflow:
                extra = ", overflow"
        return f"BT::Status({conn_name}{extra})"

    if sub == 9:  # SendData(Message)
        length, pos = read_vec_len(data, pos)
        return f"BT::SendData({length}B)"

    if sub == 10:  # SendDataResponse
        resp, pos = read_varint(data, pos)
        return f"BT::SendDataResponse({SEND_DATA_RESPONSE.get(resp, f'?{resp}')})"

    if sub == 12:  # ReceivedData(Message)
        length, pos = read_vec_len(data, pos)
        return f"BT::ReceivedData({length}B)"

    if sub == 15:  # AckFirmwareVersion
        version, pos = read_string(data, pos)
        return f"BT::AckFirmwareVersion({version})"

    if sub == 17:  # AckBtAddress
        addr, pos = read_bytes(data, pos, 6)
        return "BT::AckBtAddress(" + ":".join(f"{b:02X}" for b in addr) + ")"

    if sub == 18:  # SetTxPower
        power, pos = read_varint(data, pos)
        return f"BT::SetTxPower({TX_POWER.get(power, f'?{power}')})"

    if sub == 21:  # AckDeviceId
        dev_id, pos = read_bytes(data, pos, 8)
        return f"BT::AckDeviceId({dev_id.hex()})"

    if sub == 24:  # SetDeviceName
        name, pos = read_string(data, pos)
        return f"BT::SetDeviceName({name})"

    if sub == 26:  # Echo
        length, pos = read_vec_len(data, pos)
        return f"BT::Echo({length}B)"

    if sub == 27:  # EchoResponse
        length, pos = read_vec_len(data, pos)
        return f"BT::EchoResponse({length}B)"

    return f"BT::?{sub}"


def decode_bootloader(data, pos):
    """Decode a Bootloader sub-message starting at *pos* (after top-level discriminant 1)."""
    sub, pos = read_varint(data, pos)

    if sub in _BL_SIMPLE:
        return f"BL::{_BL_SIMPLE[sub]}"

    if sub == 5:  # AckVerifyFirmware { result, hash }
        ok, pos = read_bool(data, pos)
        hash_bytes, pos = read_bytes(data, pos, 32)
        return f"BL::AckVerifyFirmware(ok={ok}, hash={hash_bytes[:4].hex()}...)"

    if sub == 6:  # NackWithIdx
        idx, pos = read_varint(data, pos)
        return f"BL::NackWithIdx({idx})"

    if sub == 7:  # AckWithIdx
        idx, pos = read_varint(data, pos)
        return f"BL::AckWithIdx({idx})"

    if sub == 8:  # AckWithIdxCrc
        idx, pos = read_varint(data, pos)
        crc, pos = read_varint(data, pos)
        return f"BL::AckWithIdxCrc(idx={idx}, crc=0x{crc:08X})"

    if sub == 9:  # WriteFirmwareBlock
        idx, pos = read_varint(data, pos)
        length, pos = read_vec_len(data, pos)
        return f"BL::WriteFirmwareBlock(idx={idx}, {length}B)"

    if sub == 10:  # FirmwareOutOfBounds
        idx, pos = read_varint(data, pos)
        return f"BL::FirmwareOutOfBounds({idx})"

    if sub == 13:  # AckFirmwareVersion
        version, pos = read_string(data, pos)
        return f"BL::AckFirmwareVersion({version})"

    if sub == 15:  # AckBootloaderVersion
        version, pos = read_string(data, pos)
        return f"BL::AckBootloaderVersion({version})"

    if sub == 16:  # ChallengeSet { secret: [u32; 8] }
        vals = []
        for _ in range(8):
            v, pos = read_varint(data, pos)
            vals.append(v)
        return "BL::ChallengeSet"

    if sub == 17:  # AckChallengeSet
        result, pos = read_varint(data, pos)
        return f"BL::AckChallengeSet({SECRET_SAVE_RESPONSE.get(result, f'?{result}')})"

    if sub == 18:  # BootFirmware
        trust, pos = read_varint(data, pos)
        return f"BL::BootFirmware({TRUST_LEVEL.get(trust, f'?{trust}')})"

    return f"BL::?{sub}"


# ---------------------------------------------------------------------------
# Top-level message decoder
# ---------------------------------------------------------------------------

def decode_message(data):
    """Decode a postcard-encoded HostProtocolMessage.

    Returns a human-readable string, or ``None`` if the data does not look
    like a valid message.
    """
    if not data:
        return None
    try:
        pos = 0
        disc, pos = read_varint(data, pos)

        if disc == 0:
            return decode_bluetooth(data, pos)
        if disc == 1:
            return decode_bootloader(data, pos)
        if disc == 2:
            return "Reset"
        if disc == 3:
            return "GetState"
        if disc == 4:  # AckState(State)
            state, pos = read_varint(data, pos)
            return f"AckState({STATE.get(state, f'?{state}')})"
        if disc == 5:  # ChallengeRequest { nonce: u64 }
            nonce, pos = read_varint(data, pos)
            return f"ChallengeRequest(nonce=0x{nonce:X})"
        if disc == 6:  # ChallengeResult { result: [u8; 32] }
            return "ChallengeResult"
        if disc == 7:  # PostcardError
            err, pos = read_varint(data, pos)
            return f"PostcardError({POSTCARD_ERROR.get(err, f'?{err}')})"
        if disc == 8:  # InappropriateMessage(State)
            state, pos = read_varint(data, pos)
            return f"InappropriateMessage({STATE.get(state, f'?{state}')})"
        return None
    except (ValueError, IndexError):
        return None


# ---------------------------------------------------------------------------
# Saleae HLA entry point
# ---------------------------------------------------------------------------

class HostProtocolAnalyzer(HighLevelAnalyzer):
    """High Level Analyzer for host-protocol SPI traffic."""

    result_types = {
        "request":  {"format": "REQ: {{data.message}}"},
        "response": {"format": "RSP: {{data.message}}"},
        "error":    {"format": "ERR: {{data.error}}"},
    }

    def __init__(self):
        self._reset()

    def _reset(self):
        self.mosi = bytearray()
        self.miso = bytearray()
        self.start_time = None
        self.end_time = None

    # ---- frame handler ----------------------------------------------------

    def decode(self, frame):
        if frame.type == "enable":
            self._reset()
            self.start_time = frame.start_time
            return

        if frame.type == "result":
            mosi = frame.data.get("mosi", b"")
            miso = frame.data.get("miso", b"")
            if isinstance(mosi, bytes):
                self.mosi.extend(mosi)
            if isinstance(miso, bytes):
                self.miso.extend(miso)
            self.end_time = frame.end_time
            return

        if frame.type == "disable":
            if self.start_time is None:
                return
            self.end_time = frame.end_time
            result = self._process_transaction()
            self._reset()
            return result

    # ---- transaction processing -------------------------------------------

    def _process_transaction(self):
        if not self.mosi and not self.miso:
            return None

        # Detect target firmware from the first MISO byte on request transactions.
        # 0x69 = Bootloader, 0x51 = Application.
        # The remaining MISO bytes may contain residual shift-register data.
        target = ""
        miso_idle = not self.miso or all(b == 0 for b in self.miso)

        if not miso_idle and self.miso and self.miso[0] in MISO_TARGET:
            # First byte is a known identifier — this is a request transaction.
            # (These values as a BE length prefix would imply >20 KB, far above
            # the 270-byte MAX_MSG_SIZE, so there is no ambiguity with responses.)
            target = MISO_TARGET[self.miso[0]]
            miso_idle = True

        if miso_idle:
            # Request transaction: MOSI carries the postcard message, MISO is idle.
            msg = decode_message(bytes(self.mosi))
            if msg:
                return AnalyzerFrame("request", self.start_time, self.end_time, {
                    "message": msg,
                    "target": target,
                })
        else:
            # Response transaction: MISO carries 2-byte BE length prefix + postcard payload.
            if len(self.miso) >= 3:
                length = (self.miso[0] << 8) | self.miso[1]
                if 0 < length <= len(self.miso) - 2:
                    payload = bytes(self.miso[2:2 + length])
                    msg = decode_message(payload)
                    if msg:
                        return AnalyzerFrame("response", self.start_time, self.end_time, {
                            "message": msg,
                        })

            # MISO was non-zero but didn't decode as a response — fall back to MOSI.
            msg = decode_message(bytes(self.mosi))
            if msg:
                return AnalyzerFrame("request", self.start_time, self.end_time, {
                    "message": msg,
                    "target": target,
                })

        # Nothing decoded — emit an error bubble for non-trivial transactions.
        mosi_nz = any(b != 0 for b in self.mosi)
        miso_nz = any(b != 0 for b in self.miso)
        if mosi_nz or miso_nz:
            parts = []
            if mosi_nz:
                parts.append(f"MOSI {len(self.mosi)}B")
            if miso_nz:
                parts.append(f"MISO {len(self.miso)}B")
            return AnalyzerFrame("error", self.start_time, self.end_time, {
                "error": f"undecoded ({', '.join(parts)})",
            })

        return None
