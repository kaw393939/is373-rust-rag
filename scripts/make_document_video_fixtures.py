"""Generate deterministic local QA documents and a narrated video; no cloud services.

Requires reportlab, pypdf, Pillow, FFmpeg and macOS say. PDF files are test fixtures,
not deliverable documents. Run from repository root with the bundled Python or a venv.
"""
from pathlib import Path
import subprocess
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor
from PIL import Image, ImageDraw, ImageFont
from pypdf import PdfReader, PdfWriter

root=Path(__file__).resolve().parents[1]
pdf=root/'tmp/pdfs'; media=root/'data/qa/compound'
pdf.mkdir(parents=True,exist_ok=True); media.mkdir(parents=True,exist_ok=True)

def heading(c,title,subtitle,page):
    c.setFillColor(HexColor('#18483d')); c.rect(0,730,612,62,fill=1,stroke=0)
    c.setFillColor(HexColor('#ffffff')); c.setFont('Helvetica-Bold',24); c.drawString(44,754,title)
    c.setFillColor(HexColor('#263b35')); c.setFont('Helvetica',12); c.drawString(44,694,subtitle)
    c.setFont('Helvetica',9); c.drawString(44,30,f'Synthetic teaching fixture | Page {page}')

c=canvas.Canvas(str(pdf/'workshop.pdf'),pagesize=(612,792))
heading(c,'ATLAS LAB','Workshop brief / Location',1)
c.setFont('Helvetica-Bold',18); c.drawString(44,632,'Room: Cedar 204')
c.setFont('Helvetica',15); c.drawString(44,590,'Host: Avery Stone')
c.drawString(44,552,'See page 2 for the schedule and supplies.'); c.showPage()
heading(c,'ATLAS LAB','Workshop brief / Schedule and supplies',2)
for y,text in [(632,'The workshop begins Tuesday at 10:00 AM.'),(584,'Bring an orange notebook.'),(536,'Project code: ATLAS-58.')]:
    c.setFont('Helvetica',17); c.drawString(44,y,text)
c.save()

font_path='/System/Library/Fonts/Supplemental/Arial Bold.ttf'
large=ImageFont.truetype(font_path,56); medium=ImageFont.truetype(font_path,34)
scan=Image.new('RGB',(900,1100),'#f8f7f3'); d=ImageDraw.Draw(scan)
d.rectangle((50,50,850,190),fill='#18483d');d.text((90,90),'ORBIT STUDY',font=large,fill='white')
d.text((65,340),'The launch color is',font=medium,fill='#25382e')
d.text((65,410),'PURPLE',font=large,fill='#712b9e');d.text((65,600),'Badge number: 42',font=medium,fill='#25382e')
scan.save(media/'scan.png')
c=canvas.Canvas(str(pdf/'scanned.pdf'),pagesize=(612,792));c.drawImage(str(media/'scan.png'),0,0,width=612,height=792);c.save()

writer=PdfWriter();writer.append(PdfReader(pdf/'workshop.pdf'));writer.encrypt('fixture-only');writer.write(pdf/'locked.pdf')
c=canvas.Canvas(str(pdf/'too-many-pages.pdf'));[c.showPage() for _ in range(21)];c.save()
c=canvas.Canvas(str(pdf/'too-much-text.pdf'))
for page in range(12):
    t=c.beginText(40,780);t.setFont('Helvetica',10)
    for _ in range(30):t.textLine('Synthetic capacity test sentence with many repeated words for bounded extraction.')
    c.drawText(t);c.showPage()
c.save()
(pdf/'corrupt.pdf').write_bytes(b'%PDF-1.7\nnot a PDF document')

colors=[('#d53b34','RED SETUP'),('#246bd3','BLUE REVIEW'),('#23985b','GREEN READY')]
for i,(color,label) in enumerate(colors):
    img=Image.new('RGB',(640,360),'#f7f6f2');d=ImageDraw.Draw(img)
    d.rectangle((70,44,570,220),fill=color);d.text((45,265),label,font=medium,fill='#152e27');img.save(media/f'scene-{i}.png')
subprocess.run(['say','-v','Samantha','-o',str(media/'narration.aiff'),'The demo starts on Tuesday at ten in the morning. The access word is cedar.'],check=True)
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(media/'narration.aiff'),'-ar','16000','-ac','1',str(media/'narration.wav')],check=True)
for i in range(3):
    subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-loop','1','-i',str(media/f'scene-{i}.png'),'-t','3','-r','12','-c:v','libx264','-pix_fmt','yuv420p',str(media/f'clip-{i}.mp4')],check=True)
(media/'concat.txt').write_text(''.join(f"file 'clip-{i}.mp4'\n" for i in range(3)))
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-f','concat','-safe','1','-i',str(media/'concat.txt'),'-c','copy',str(media/'silent.mp4')],check=True)
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(media/'silent.mp4'),'-i',str(media/'narration.wav'),'-map','0:v:0','-map','1:a:0','-c:v','copy','-af','apad','-c:a','aac','-t','9',str(media/'narrated.mp4')],check=True)
subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-i',str(media/'silent.mp4'),'-c:v','libvpx-vp9','-b:v','150k','-live','1',str(media/'browser.webm')],check=True)
print('Created PDF fixtures in tmp/pdfs and video fixtures in data/qa/compound')
