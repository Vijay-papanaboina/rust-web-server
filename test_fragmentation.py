import socket
import random
import time

def make_frame(opcode, payload, is_fin=True):
    if isinstance(payload, str):
        payload = payload.encode('utf-8')
    
    # Byte 0: FIN (1 bit) + RSV (3 bits) + Opcode (4 bits)
    b0 = (0x80 if is_fin else 0x00) | (opcode & 0x0F)
    
    # Byte 1: Mask (1 bit, must be 1 from client) + Payload len (7 bits)
    length = len(payload)
    if length <= 125:
        header = bytes([b0, 0x80 | length])
    elif length <= 65535:
        header = bytes([b0, 0x80 | 126]) + length.to_bytes(2, byteorder='big')
    else:
        header = bytes([b0, 0x80 | 127]) + length.to_bytes(8, byteorder='big')
    
    # Generate 4-byte random masking key
    mask = bytes(random.getrandbits(8) for _ in range(4))
    
    # Mask the payload
    masked_payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    
    return header + mask + masked_payload

def main():
    host = "127.0.0.1"
    port = 7878
    
    print(f"Connecting to {host}:{port}...")
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect((host, port))
    
    # Send HTTP Upgrade Handshake
    handshake = (
        "GET /ws HTTP/1.1\r\n"
        f"Host: {host}:{port}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n"
        "Sec-WebSocket-Version: 13\r\n\r\n"
    )
    s.sendall(handshake.encode('utf-8'))
    
    # Read the upgrade response from server
    response = s.recv(4096)
    print("Handshake Response:")
    print(response.decode('utf-8', errors='ignore'))
    
    print("Sending Fragment 1 (Opcode: Text, FIN: False, Payload: 'Hello ')...")
    s.sendall(make_frame(opcode=1, payload="Hello ", is_fin=False))
    time.sleep(0.5)
    
    print("Sending Fragment 2 (Opcode: Continuation, FIN: False, Payload: 'beautiful ')...")
    s.sendall(make_frame(opcode=0, payload="beautiful ", is_fin=False))
    time.sleep(0.5)
    
    print("Sending Fragment 3 (Opcode: Continuation, FIN: True, Payload: 'fragmented world!')...")
    s.sendall(make_frame(opcode=0, payload="fragmented world!", is_fin=True))
    time.sleep(0.5)
    
    print("Sending Close Frame...")
    s.sendall(make_frame(opcode=8, payload="", is_fin=True))
    time.sleep(0.5)
    
    s.close()
    print("Done!")

if __name__ == "__main__":
    main()
