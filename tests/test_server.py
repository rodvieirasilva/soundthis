#!/usr/bin/env python3
"""
Unit and integration tests for SoundThis server.
"""

import asyncio
import base64
import json
import os
import struct
import sys
import unittest
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from server import SoundThisServer, make_ws_frame, read_ws_frame, WS_GUID

TEST_PORT = 19698

class TestSoundThisServer(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.server = SoundThisServer(host="127.0.0.1", port=TEST_PORT, initial_enabled=True)
        self.tcp_server = await asyncio.start_server(
            self.server.handle_request,
            self.server.host,
            self.server.port,
            reuse_address=True
        )

    async def asyncTearDown(self):
        if self.server.audio_proc:
            try:
                self.server.audio_proc.terminate()
            except Exception:
                pass
        self.tcp_server.close()
        await self.tcp_server.wait_closed()

    async def test_cors_options(self):
        reader, writer = await asyncio.open_connection("127.0.0.1", TEST_PORT)
        writer.write(b"OPTIONS /api/status HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        await writer.drain()

        resp = await reader.read(1024)
        writer.close()
        text = resp.decode()
        self.assertIn("HTTP/1.1 204 No Content", text)
        self.assertIn("Access-Control-Allow-Origin: *", text)
        self.assertIn("Access-Control-Allow-Methods: GET, POST, OPTIONS", text)

    async def test_api_status_and_toggle(self):
        # 1. Check status
        reader, writer = await asyncio.open_connection("127.0.0.1", TEST_PORT)
        writer.write(b"GET /api/status HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        await writer.drain()
        hdr = await reader.readuntil(b"\r\n\r\n")
        body = await reader.read(2048)
        writer.close()

        self.assertIn(b"Access-Control-Allow-Origin: *", hdr)
        data = json.loads(body.decode())
        self.assertTrue(data["enabled"])
        self.assertEqual(data["port"], TEST_PORT)

        # 2. Toggle to disabled
        reader, writer = await asyncio.open_connection("127.0.0.1", TEST_PORT)
        writer.write(b"POST /api/toggle HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        await writer.drain()
        hdr = await reader.readuntil(b"\r\n\r\n")
        body = await reader.read(2048)
        writer.close()

        data = json.loads(body.decode())
        self.assertFalse(data["enabled"])

        # 3. Enable
        reader, writer = await asyncio.open_connection("127.0.0.1", TEST_PORT)
        writer.write(b"POST /api/enable HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        await writer.drain()
        hdr = await reader.readuntil(b"\r\n\r\n")
        body = await reader.read(2048)
        writer.close()

        data = json.loads(body.decode())
        self.assertTrue(data["enabled"])

    async def test_static_html(self):
        reader, writer = await asyncio.open_connection("127.0.0.1", TEST_PORT)
        writer.write(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        await writer.drain()
        hdr = await reader.readuntil(b"\r\n\r\n")
        body = await reader.read(32768)
        writer.close()

        self.assertIn(b"HTTP/1.1 200 OK", hdr)
        self.assertIn(b"text/html", hdr)
        self.assertIn(b"Access-Control-Allow-Origin: *", hdr)
        self.assertIn(b"SoundThis", body)
        self.assertIn(b"soundthis_client_name", body)

    async def test_websocket_client_and_identify(self):
        key = base64.b64encode(b"0123456789abcdef").decode()
        reader, writer = await asyncio.open_connection("127.0.0.1", TEST_PORT)
        req = (
            f"GET /ws HTTP/1.1\r\n"
            f"Host: 127.0.0.1:{TEST_PORT}\r\n"
            f"Upgrade: websocket\r\n"
            f"Connection: Upgrade\r\n"
            f"Sec-WebSocket-Key: {key}\r\n"
            f"Sec-WebSocket-Version: 13\r\n\r\n"
        )
        writer.write(req.encode())
        await writer.drain()
        resp_hdr = await reader.readuntil(b"\r\n\r\n")
        self.assertIn(b"101 Switching Protocols", resp_hdr)

        # Server sends initial status
        opcode, payload = await read_ws_frame(reader)
        self.assertEqual(opcode, 1)
        status_msg = json.loads(payload.decode())
        self.assertEqual(status_msg["type"], "status")
        self.assertTrue(status_msg["enabled"])

        # Client sends identify name (masked)
        client_name = "Living Room TV"
        msg = json.dumps({"type": "identify", "name": client_name}).encode()
        mask = b"\xaa\xbb\xcc\xdd"
        masked = bytearray(len(msg))
        for i in range(len(msg)):
            masked[i] = msg[i] ^ mask[i % 4]
        frame = bytearray([0x81, 0x80 | len(msg)]) + mask + masked
        writer.write(frame)
        await writer.drain()

        await asyncio.sleep(0.05)
        self.assertEqual(len(self.server.ws_clients), 1)
        client_info = list(self.server.ws_clients.values())[0]
        self.assertEqual(client_info["name"], client_name)

        # Close frame
        writer.write(bytearray([0x88, 0x80]) + mask)
        await writer.drain()
        await asyncio.sleep(0.05)
        writer.close()

if __name__ == "__main__":
    unittest.main()
