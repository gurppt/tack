"""Create original 16px hard-edged starter icons, never replace edited PNGs."""
from pathlib import Path
from PIL import Image,ImageDraw
ROOT=Path(__file__).resolve().parents[1]/'gfx/icons'
def main():
 ROOT.mkdir(parents=True,exist_ok=True)
 names=['pointer','pan','text','rectangle','line','arrow','scribble','frame','duplicate','undo','redo','share','join','save','grid','placeholder']
 for name in names:
  path=ROOT/(name+'.png')
  if path.exists():continue
  im=Image.new('RGBA',(16,16));d=ImageDraw.Draw(im);c=(64,224,208,255);a=(255,20,147,255)
  if name=='pointer':d.polygon([(3,2),(3,12),(6,9),(9,13),(11,12),(8,8),(12,8)],fill=c)
  elif name=='pan':
   d.line([(3,8),(12,8)],fill=c,width=1);d.line([(8,3),(8,12)],fill=c,width=1)
   for pts in [[(3,8),(5,6),(5,10)],[(12,8),(10,6),(10,10)],[(8,3),(6,5),(10,5)],[(8,12),(6,10),(10,10)]]:d.polygon(pts,fill=c)
  elif name=='text':d.line([(3,3),(12,3)],fill=c,width=2);d.line([(8,3),(8,12)],fill=c,width=2);d.line([(5,12),(11,12)],fill=c)
  elif name in ('rectangle','frame','duplicate'):
   d.rectangle((3,4,12,12),outline=c)
   if name=='frame':d.line([(3,2),(8,2)],fill=a)
   if name=='duplicate':d.rectangle((6,2,14,10),outline=a)
  elif name in ('line','arrow'):
   d.line([(3,12),(12,3)],fill=c)
   if name=='arrow':d.line([(7,3),(12,3),(12,8)],fill=c)
  elif name=='scribble':d.line([(2,10),(5,3),(7,11),(10,4),(13,10)],fill=c)
  elif name in ('undo','redo'):
   d.line([(3,5),(11,5),(13,7),(13,11),(7,11)],fill=c);d.polygon([(2,5),(6,2),(6,8)],fill=c)
   if name=='redo':im=im.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
  elif name in ('share','join'):
   for x,y in [(3,3),(11,7),(3,11)]:d.rectangle((x-1,y-1,x+1,y+1),fill=c)
   d.line([(4,3),(10,7),(4,11)],fill=a)
   if name=='join':d.rectangle((6,6,8,8),fill=c)
  elif name=='save':d.rectangle((3,2,12,13),outline=c);d.rectangle((5,2,10,6),outline=a);d.rectangle((5,9,10,13),outline=c)
  elif name=='grid':
   for x in range(3,14,4):
    for y in range(3,14,4):d.point((x,y),fill=c)
  else:d.rectangle((3,3,12,12),outline=c);d.line((3,3,12,12),fill=a)
  im.save(path,optimize=True)
if __name__=='__main__':main()
