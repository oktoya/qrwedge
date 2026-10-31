"""아이콘 생성: assets/icon.png(1024px, 맥용) + assets/icon.ico(Windows용). 바꿀 때만 실행한다.
필요: pip install pillow  (Windows 글꼴 Segoe UI Bold 사용)
"""
import random
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

S = 1024
NAVY = (22, 50, 92)
WHITE = (255, 255, 255)
RED = (235, 64, 52)
here = Path(__file__).parent

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle((32, 32, S - 32, S - 32), radius=200, fill=NAVY)

# QR 카드: 흰 바탕에 남색 모듈 (15x15 단순화한 QR)
card = (232, 96, 792, 656)
d.rounded_rectangle(card, radius=48, fill=WHITE)
n, pad = 15, 48
m = (card[2] - card[0] - pad * 2) / n
ox, oy = card[0] + pad, card[1] + pad


def cell(cx, cy, w=1, h=1, color=NAVY):
    d.rectangle((ox + cx * m, oy + cy * m, ox + (cx + w) * m - 1, oy + (cy + h) * m - 1), fill=color)


def finder(cx, cy):
    cell(cx, cy, 5, 5)
    cell(cx + 1, cy + 1, 3, 3, WHITE)
    cell(cx + 2, cy + 2)


finders = [(0, 0), (n - 5, 0), (0, n - 5)]
for f in finders:
    finder(*f)
rng = random.Random(7)
for y in range(n):
    for x in range(n):
        near_finder = any(fx - 1 <= x <= fx + 5 and fy - 1 <= y <= fy + 5 for fx, fy in finders)
        if not near_finder and rng.random() < 0.5:
            cell(x, y)

# 스캔 라인
d.rounded_rectangle((card[0] - 28, 366, card[2] + 28, 386), radius=10, fill=RED)

# 숫자
font = ImageFont.truetype("C:/Windows/Fonts/segoeuib.ttf", 230)
d.text((S / 2, 832), "123", font=font, fill=WHITE, anchor="mm")

img.save(here / "icon.png")
img.save(here / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("ok")
