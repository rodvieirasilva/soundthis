#!/usr/bin/env python3
"""
SoundThis — Real-time Desktop Audio Streaming Server for Omarchy
Captures system audio monitor and streams PCM over WebSocket and HTTP.
"""

import argparse
import asyncio
import base64
import hashlib
import json
import os
import socket
import struct
import sys
import time

WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
SAMPLE_RATE = 48000
CHANNELS = 2
SAMPLE_WIDTH = 2  # 16-bit
BYTES_PER_FRAME = CHANNELS * SAMPLE_WIDTH  # 4 bytes
# 20ms chunks = 48000 * 0.02 = 960 frames = 3840 bytes
CHUNK_FRAMES = 960
CHUNK_BYTES = CHUNK_FRAMES * BYTES_PER_FRAME

def get_lan_ip():
    """Detect LAN IP address for external device access."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(("8.8.8.8", 80))
        ip = s.getsockname()[0]
    except Exception:
        ip = "127.0.0.1"
    finally:
        s.close()
    return ip

def make_ws_frame(data: bytes, opcode: int = 2) -> bytes:
    """Encode binary or text WebSocket frame from server to client."""
    length = len(data)
    header = bytearray([0x80 | (opcode & 0x0F)])
    if length < 126:
        header.append(length)
    elif length < 65536:
        header.append(126)
        header.extend(struct.pack("!H", length))
    else:
        header.append(127)
        header.extend(struct.pack("!Q", length))
    return bytes(header) + data

async def read_ws_frame(reader: asyncio.StreamReader):
    """Decode incoming masked WebSocket frame from client."""
    header = await reader.readexactly(2)
    b1, b2 = header[0], header[1]
    fin = (b1 & 0x80) != 0
    opcode = b1 & 0x0F
    has_mask = (b2 & 0x80) != 0
    length = b2 & 0x7F

    if length == 126:
        ext = await reader.readexactly(2)
        length = struct.unpack("!H", ext)[0]
    elif length == 127:
        ext = await reader.readexactly(8)
        length = struct.unpack("!Q", ext)[0]

    mask_key = b""
    if has_mask:
        mask_key = await reader.readexactly(4)

    payload = await reader.readexactly(length)
    if has_mask:
        unmasked = bytearray(length)
        for i in range(length):
            unmasked[i] = payload[i] ^ mask_key[i % 4]
        payload = bytes(unmasked)

    return opcode, payload

def create_wav_header(sample_rate=SAMPLE_RATE, channels=CHANNELS, bits_per_sample=16):
    """Generate a standard 44-byte WAV header for streaming (infinite length)."""
    byte_rate = sample_rate * channels * (bits_per_sample // 8)
    block_align = channels * (bits_per_sample // 8)
    data_size = 0x7FFFFFFF  # Max signed 32-bit int (~2GB stream indicator)
    file_size = data_size + 36

    header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",
        file_size,
        b"WAVE",
        b"fmt ",
        16,              # Subchunk1Size for PCM
        1,               # AudioFormat (1 = PCM)
        channels,
        sample_rate,
        byte_rate,
        block_align,
        bits_per_sample,
        b"data",
        data_size
    )
    return header

class SoundThisServer:
    def __init__(self, host="0.0.0.0", port=9697, web_dir=None, initial_enabled=True):
        self.host = host
        self.port = port
        self.lan_ip = get_lan_ip()
        self.enabled = initial_enabled
        if not web_dir:
            base = os.path.dirname(os.path.abspath(__file__))
            candidates = [
                os.path.join(base, "web"),
                os.path.expanduser("~/.local/share/soundthis/web"),
                "/home/rvs/dev/soundthis/web"
            ]
            self.web_dir = next((d for d in candidates if os.path.isdir(d)), os.path.join(base, "web"))
        else:
            self.web_dir = web_dir
        self.ws_clients = {}  # client_id -> {"writer": writer, "name": str, "ip": str, "connected_at": float}
        self.http_queues = set()  # set of asyncio.Queue for /stream.wav
        self.audio_proc = None
        self.capture_task = None
        self.server = None

    def status_dict(self):
        clients = []
        now = time.time()
        for cid, info in self.ws_clients.items():
            clients.append({
                "id": cid,
                "name": info["name"],
                "ip": info["ip"],
                "uptime": int(now - info["connected_at"])
            })
        return {
            "enabled": self.enabled,
            "port": self.port,
            "lan_ip": self.lan_ip,
            "clients_count": len(clients),
            "clients": clients,
            "capturing": self.audio_proc is not None
        }

    async def broadcast_status(self):
        """Notify all connected WebSocket clients of status change."""
        msg = json.dumps({"type": "status", "enabled": self.enabled}).encode("utf-8")
        frame = make_ws_frame(msg, opcode=1)
        dead = []
        for cid, info in list(self.ws_clients.items()):
            try:
                info["writer"].write(frame)
                await info["writer"].drain()
            except Exception:
                dead.append(cid)
        for cid in dead:
            self.remove_client(cid)

    def should_capture(self):
        return self.enabled and (len(self.ws_clients) > 0 or len(self.http_queues) > 0)

    def ensure_capture_state(self):
        if self.should_capture():
            if self.capture_task is None or self.capture_task.done():
                self.capture_task = asyncio.create_task(self.audio_loop())
        else:
            if self.audio_proc:
                try:
                    self.audio_proc.terminate()
                except Exception:
                    pass
                self.audio_proc = None

    async def audio_loop(self):
        """Capture monitor audio and distribute chunks to WS and HTTP clients."""
        print(f"[*] Starting audio capture from @DEFAULT_MONITOR@...")
        try:
            self.audio_proc = await asyncio.create_subprocess_exec(
                "parec", "-r", "-d", "@DEFAULT_MONITOR@",
                f"--rate={SAMPLE_RATE}", "--format=s16le", f"--channels={CHANNELS}",
                "--latency-msec=20",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.DEVNULL
            )
        except Exception as e:
            print(f"[!] Failed to launch parec: {e}")
            self.audio_proc = None
            return

        try:
            while self.should_capture() and self.audio_proc and self.audio_proc.returncode is None:
                chunk = await self.audio_proc.stdout.read(CHUNK_BYTES)
                if not chunk:
                    break

                # 1. Send to WebSocket clients
                if self.ws_clients:
                    frame = make_ws_frame(chunk, opcode=2)
                    dead_ws = []
                    for cid, info in list(self.ws_clients.items()):
                        try:
                            info["writer"].write(frame)
                        except Exception:
                            dead_ws.append(cid)
                    for cid in dead_ws:
                        self.remove_client(cid)

                # 2. Send to HTTP stream queues
                if self.http_queues:
                    dead_queues = set()
                    for q in list(self.http_queues):
                        try:
                            q.put_nowait(chunk)
                        except asyncio.QueueFull:
                            # Drop frame if client queue is saturated to keep low latency
                            pass
                        except Exception:
                            dead_queues.add(q)
                    self.http_queues.difference_update(dead_queues)

                # Yield control briefly to event loop
                await asyncio.sleep(0.001)

        except asyncio.CancelledError:
            pass
        except Exception as e:
            print(f"[!] Audio loop exception: {e}")
        finally:
            if self.audio_proc:
                try:
                    self.audio_proc.terminate()
                    await self.audio_proc.wait()
                except Exception:
                    pass
                self.audio_proc = None
            print("[*] Audio capture stopped.")

    def add_client(self, client_id, writer, name, ip):
        self.ws_clients[client_id] = {
            "writer": writer,
            "name": name,
            "ip": ip,
            "connected_at": time.time()
        }
        self.ensure_capture_state()

    def remove_client(self, client_id):
        self.ws_clients.pop(client_id, None)
        self.ensure_capture_state()

    async def handle_request(self, reader, writer):
        """HTTP and WebSocket request router."""
        peer_name = writer.get_extra_info("peername")
        client_ip = peer_name[0] if peer_name else "unknown"

        try:
            line = await reader.readline()
            if not line:
                writer.close()
                return
            req_line = line.decode("utf-8", errors="ignore").strip()
            parts = req_line.split()
            if len(parts) < 2:
                writer.close()
                return
            method, raw_path = parts[0].upper(), parts[1]
            path = raw_path.split("?")[0]

            headers = {}
            while True:
                hline = await reader.readline()
                if not hline or hline == b"\r\n":
                    break
                hstr = hline.decode("utf-8", errors="ignore").strip()
                if ":" in hstr:
                    k, v = hstr.split(":", 1)
                    headers[k.strip().lower()] = v.strip()

            cors = (
                "Access-Control-Allow-Origin: *\r\n"
                "Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n"
                "Access-Control-Allow-Headers: *\r\n"
            )

            # CORS Preflight
            if method == "OPTIONS":
                resp = (
                    "HTTP/1.1 204 No Content\r\n"
                    + cors
                    + "Access-Control-Max-Age: 86400\r\n"
                    + "Content-Length: 0\r\n\r\n"
                )
                writer.write(resp.encode("utf-8"))
                await writer.drain()
                writer.close()
                return

            # API: GET Status
            if path == "/api/status" and method == "GET":
                body = json.dumps(self.status_dict(), indent=2).encode("utf-8")
                resp = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    + cors
                    + f"Content-Length: {len(body)}\r\n\r\n"
                ).encode("utf-8") + body
                writer.write(resp)
                await writer.drain()
                writer.close()
                return

            # API: POST Toggle
            if path == "/api/toggle" and method == "POST":
                self.enabled = not self.enabled
                self.ensure_capture_state()
                await self.broadcast_status()
                body = json.dumps(self.status_dict()).encode("utf-8")
                resp = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    + cors
                    + f"Content-Length: {len(body)}\r\n\r\n"
                ).encode("utf-8") + body
                writer.write(resp)
                await writer.drain()
                writer.close()
                return

            # API: POST Enable
            if path == "/api/enable" and method == "POST":
                self.enabled = True
                self.ensure_capture_state()
                await self.broadcast_status()
                body = json.dumps(self.status_dict()).encode("utf-8")
                resp = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    + cors
                    + f"Content-Length: {len(body)}\r\n\r\n"
                ).encode("utf-8") + body
                writer.write(resp)
                await writer.drain()
                writer.close()
                return

            # API: POST Disable
            if path == "/api/disable" and method == "POST":
                self.enabled = False
                self.ensure_capture_state()
                await self.broadcast_status()
                body = json.dumps(self.status_dict()).encode("utf-8")
                resp = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: application/json\r\n"
                    + cors
                    + f"Content-Length: {len(body)}\r\n\r\n"
                ).encode("utf-8") + body
                writer.write(resp)
                await writer.drain()
                writer.close()
                return

            # WebSocket Endpoint: /ws
            if path == "/ws" and headers.get("upgrade", "").lower() == "websocket":
                key = headers.get("sec-websocket-key", "")
                accept = base64.b64encode(hashlib.sha1((key + WS_GUID).encode("ascii")).digest()).decode("ascii")
                handshake = (
                    "HTTP/1.1 101 Switching Protocols\r\n"
                    "Upgrade: websocket\r\n"
                    "Connection: Upgrade\r\n"
                    f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
                ).encode("utf-8")
                writer.write(handshake)
                await writer.drain()

                client_id = f"client_{int(time.time()*1000)}_{id(writer)}"
                default_name = f"Client {client_ip}"
                self.add_client(client_id, writer, default_name, client_ip)

                # Send initial status
                status_frame = make_ws_frame(json.dumps({"type": "status", "enabled": self.enabled}).encode("utf-8"), opcode=1)
                writer.write(status_frame)
                await writer.drain()

                try:
                    while True:
                        opcode, payload = await read_ws_frame(reader)
                        if opcode == 8:  # Close
                            break
                        elif opcode == 9:  # Ping
                            writer.write(make_ws_frame(payload, opcode=10))
                            await writer.drain()
                        elif opcode == 1:  # Text frame
                            try:
                                msg = json.loads(payload.decode("utf-8"))
                                if msg.get("type") in ("identify", "setName"):
                                    name = msg.get("name", "").strip()
                                    if name and client_id in self.ws_clients:
                                        self.ws_clients[client_id]["name"] = name
                            except Exception:
                                pass
                except (asyncio.IncompleteReadError, ConnectionResetError, BrokenPipeError):
                    pass
                finally:
                    self.remove_client(client_id)
                    try:
                        writer.close()
                        await writer.wait_closed()
                    except Exception:
                        pass
                return

            # HTTP Audio Stream: /stream.wav
            if path in ("/stream.wav", "/stream"):
                resp = (
                    "HTTP/1.1 200 OK\r\n"
                    "Content-Type: audio/wav\r\n"
                    "Cache-Control: no-cache, no-store\r\n"
                    + cors
                    + "Transfer-Encoding: chunked\r\n\r\n"
                ).encode("utf-8")
                writer.write(resp)

                # Send WAV header as first chunk
                wav_hdr = create_wav_header()
                writer.write(f"{len(wav_hdr):X}\r\n".encode("ascii") + wav_hdr + b"\r\n")
                await writer.drain()

                queue = asyncio.Queue(maxsize=20)
                self.http_queues.add(queue)
                self.ensure_capture_state()

                try:
                    while True:
                        pcm_chunk = await queue.get()
                        chunk_msg = f"{len(pcm_chunk):X}\r\n".encode("ascii") + pcm_chunk + b"\r\n"
                        writer.write(chunk_msg)
                        await writer.drain()
                except (ConnectionResetError, BrokenPipeError):
                    pass
                finally:
                    self.http_queues.discard(queue)
                    self.ensure_capture_state()
                    try:
                        writer.close()
                        await writer.wait_closed()
                    except Exception:
                        pass
                return

            # Static Web Assets (HTML client)
            if path in ("/", "/index.html"):
                index_path = os.path.join(self.web_dir, "index.html")
                if os.path.isfile(index_path):
                    with open(index_path, "rb") as f:
                        body = f.read()
                    resp = (
                        "HTTP/1.1 200 OK\r\n"
                        "Content-Type: text/html; charset=utf-8\r\n"
                        + cors
                        + f"Content-Length: {len(body)}\r\n\r\n"
                    ).encode("utf-8") + body
                    writer.write(resp)
                    await writer.drain()
                    writer.close()
                    return

            # Fallback 404
            not_found = b"404 Not Found"
            resp = (
                "HTTP/1.1 404 Not Found\r\n"
                "Content-Type: text/plain\r\n"
                + cors
                + f"Content-Length: {len(not_found)}\r\n\r\n"
            ).encode("utf-8") + not_found
            writer.write(resp)
            await writer.drain()
            writer.close()

        except Exception as e:
            try:
                writer.close()
            except Exception:
                pass

    async def start(self):
        self.server = await asyncio.start_server(self.handle_request, self.host, self.port, reuse_address=True)
        print("=" * 60)
        print(f"  SoundThis Server running on port {self.port}")
        print(f"  Web Client: http://localhost:{self.port} or http://{self.lan_ip}:{self.port}")
        print(f"  Status API: http://localhost:{self.port}/api/status")
        print(f"  Initial state: {'ENABLED' if self.enabled else 'DISABLED'}")
        print("=" * 60)
        async with self.server:
            await self.server.serve_forever()

def main():
    parser = argparse.ArgumentParser(description="SoundThis Audio Streaming Server")
    parser.add_argument("--host", default="0.0.0.0", help="Binding host (default 0.0.0.0)")
    parser.add_argument("--port", type=int, default=9697, help="Port (default 9697)")
    parser.add_argument("--disabled", action="store_true", help="Start in disabled state")
    args = parser.parse_args()

    server = SoundThisServer(
        host=args.host,
        port=args.port,
        initial_enabled=not args.disabled
    )

    try:
        asyncio.run(server.start())
    except KeyboardInterrupt:
        print("\n[*] SoundThis shutting down.")

if __name__ == "__main__":
    main()
