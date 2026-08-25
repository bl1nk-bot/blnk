import os
import subprocess
import time
from PIL import Image, ImageDraw, ImageFont

BG_COLOR = (18, 22, 28)
HEADER_COLOR = (30, 36, 46)
TEXT_COLOR = (220, 225, 235)
GREEN = (74, 222, 128)
BLUE = (96, 165, 250)
YELLOW = (250, 204, 21)
PURPLE = (192, 132, 252)
MUTED = (148, 163, 184)
RED = (248, 113, 113)

def get_font(size=14):
    try:
        return ImageFont.truetype("consola.ttf", size)
    except:
        try:
            return ImageFont.truetype("DejaVuSansMono.ttf", size)
        except:
            return ImageFont.load_default()

def create_terminal_base(width=780, height=440, title="blnk - remote multitool"):
    img = Image.new("RGB", (width, height), BG_COLOR)
    draw = ImageDraw.Draw(img)

    # Title bar
    draw.rectangle([(0, 0), (width, 36)], fill=HEADER_COLOR)
    # Window buttons
    draw.ellipse([(14, 12), (24, 22)], fill=(239, 68, 68))
    draw.ellipse([(32, 12), (42, 22)], fill=(234, 179, 8))
    draw.ellipse([(50, 12), (60, 22)], fill=(34, 197, 94))

    font = get_font(13)
    draw.text((width // 2 - 80, 10), title, font=font, fill=MUTED)
    return img, draw

def render_lines(draw, lines, start_y=50, start_x=20, line_height=20):
    font = get_font(13)
    y = start_y
    for item in lines:
        if isinstance(item, tuple):
            text, color = item
            draw.text((start_x, y), text, font=font, fill=color)
        elif isinstance(item, list):
            x = start_x
            for text, color in item:
                draw.text((x, y), text, font=font, fill=color)
                bbox = font.getbbox(text)
                x += (bbox[2] - bbox[0])
        y += line_height

def generate_shot1():
    img, draw = create_terminal_base(780, 480, "blnk serve --qr")
    lines = [
        ([("$ ", GREEN), ("blnk serve --qr --local-fixture", TEXT_COLOR)]),
        [("identity_uid=", MUTED), ("id_dK9sL2mP8qRt7vXyZ10w2A", PURPLE)],
        [("signaling_endpoint_configured=", MUTED), ("true", GREEN)],
        [("mDNS service announced on local LAN (type=_blnk._tcp.local)", BLUE)],
        (""),
        ("--- Pairing QR Code ---", YELLOW),
        ("  ██████████████  ████  ██████████████  ", TEXT_COLOR),
        ("  ██          ██  ██    ██          ██  ", TEXT_COLOR),
        ("  ██  ██████  ██  ████  ██  ██████  ██  ", TEXT_COLOR),
        ("  ██  ██████  ██  ██    ██  ██████  ██  ", TEXT_COLOR),
        ("  ██          ██  ████  ██          ██  ", TEXT_COLOR),
        ("  ██████████████  ██  ██  ████████████  ", TEXT_COLOR),
        ("                  ██████                ", TEXT_COLOR),
        ("  ████████  ████████  ██████  ████████  ", TEXT_COLOR),
        ("  ████  ██  ██  ██████    ██  ████  ██  ", TEXT_COLOR),
        ("  ██████████████  ██  ██  ████  ██  ██  ", TEXT_COLOR),
        ("  ██          ██  ██████  ████████████  ", TEXT_COLOR),
        ("  ██  ██████  ██    ████  ██  ██  ██    ", TEXT_COLOR),
        ("  ██████████████  ██████  ██  ██████    ", TEXT_COLOR),
        ("-----------------------", YELLOW),
        (""),
        [("fixture_url=", MUTED), ("ws://127.0.0.1:49821/ws", BLUE)],
        [("fixture_pin_configured=", MUTED), ("true", GREEN)],
        [("serve_status=", MUTED), ("running; press Ctrl-C to stop", GREEN)],
    ]
    render_lines(draw, lines)
    img.save("docs/assets/blnk-serve-qr.png")
    print("Generated docs/assets/blnk-serve-qr.png")

def generate_shot2():
    img, draw = create_terminal_base(780, 420, "blnk connect & devices --local")
    lines = [
        ([("$ ", GREEN), ("blnk devices --local", TEXT_COLOR)]),
        ("Discovering blnk peers on local network (mDNS)...", BLUE),
        ("ID                      IP              PORT    HOST", YELLOW),
        ("dev_node_macbook_pro   192.168.1.142   54210   mbp-work.local", TEXT_COLOR),
        ("dev_node_desktop_win   192.168.1.189   54211   win-dev.local", TEXT_COLOR),
        (""),
        ([("$ ", GREEN), ("blnk connect --target dev_node_desktop_win --command \"uname -a\"", TEXT_COLOR)]),
        ("[*] Resolving signaling / WebRTC endpoint for target...", MUTED),
        ("[*] WebRTC data channel established (ICE: srflx/relay, DTLS: Active)", BLUE),
        ("[*] Session authenticated via 6-digit constant-time PIN challenge", GREEN),
        ("Linux win-dev 6.6.137-blnk-amd64 #1 SMP PREEMPT_DYNAMIC x86_64 GNU/Linux", TEXT_COLOR),
        (""),
        ([("$ ", GREEN), ("blnk cp source_dataset.tar.gz remote:/opt/data/ --target dev_node_desktop_win", TEXT_COLOR)]),
        ("[+] Opening file stream (stream_id=2, max_chunk=64KB)...", MUTED),
        ("[+] Transferring: [████████████████████████████████] 100% (14.2 MB/s)", GREEN),
        ("copied source_dataset.tar.gz -> remote:/opt/data/source_dataset.tar.gz", GREEN),
    ]
    render_lines(draw, lines)
    img.save("docs/assets/blnk-connect-transfer.png")
    print("Generated docs/assets/blnk-connect-transfer.png")

def generate_gif():
    frames = []

    script_steps = [
        ("blnk connect --local-fixture --command \"echo Hello blnk!\"", [
            ([("$ ", GREEN), ("blnk", TEXT_COLOR)])
        ]),
        ("blnk connect --local-fixture --command \"echo Hello blnk!\"", [
            ([("$ ", GREEN), ("blnk connect --local-fixture", TEXT_COLOR)])
        ]),
        ("blnk connect --local-fixture --command \"echo Hello blnk!\"", [
            ([("$ ", GREEN), ("blnk connect --local-fixture --command \"echo Hello blnk!\"", TEXT_COLOR)])
        ]),
        ("Connecting...", [
            ([("$ ", GREEN), ("blnk connect --local-fixture --command \"echo Hello blnk!\"", TEXT_COLOR)]),
            ("[1/3] Initializing local WebRTC loopback harness...", MUTED)
        ]),
        ("Negotiating...", [
            ([("$ ", GREEN), ("blnk connect --local-fixture --command \"echo Hello blnk!\"", TEXT_COLOR)]),
            ("[1/3] Initializing local WebRTC loopback harness...", MUTED),
            ("[2/3] Performing constant-time PIN handshake...", BLUE)
        ]),
        ("Session Ready", [
            ([("$ ", GREEN), ("blnk connect --local-fixture --command \"echo Hello blnk!\"", TEXT_COLOR)]),
            ("[1/3] Initializing local WebRTC loopback harness...", MUTED),
            ("[2/3] Performing constant-time PIN handshake...", BLUE),
            ("[3/3] Session Ready (state=Ready, stream_id=1 PTY)...", GREEN)
        ]),
        ("Output Received", [
            ([("$ ", GREEN), ("blnk connect --local-fixture --command \"echo Hello blnk!\"", TEXT_COLOR)]),
            ("[1/3] Initializing local WebRTC loopback harness...", MUTED),
            ("[2/3] Performing constant-time PIN handshake...", BLUE),
            ("[3/3] Session Ready (state=Ready, stream_id=1 PTY)...", GREEN),
            (""),
            ("Hello blnk!", YELLOW),
            (""),
            ([("$ ", GREEN), ("_ ", TEXT_COLOR)])
        ])
    ]

    for title, lines in script_steps:
        img, draw = create_terminal_base(740, 320, "blnk demo")
        render_lines(draw, lines)
        # Duplicate frames for pause duration
        dur = 3 if "Output" in title or "Ready" in title else 1
        for _ in range(dur):
            frames.append(img)

    frames[0].save(
        "docs/assets/blnk-demo.gif",
        save_all=True,
        append_images=frames[1:],
        duration=350,
        loop=0
    )
    print("Generated docs/assets/blnk-demo.gif")

if __name__ == "__main__":
    generate_shot1()
    generate_shot2()
    generate_gif()
