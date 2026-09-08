#!/usr/bin/env python3
"""Author independent mathematical resources; no third-party SCL files are copied."""
import json, math
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'resources'
entries=[]
def add(id,name,cat,description,intervals,tags,favorite=False,aliases=[],listening_hint=""):
    scl='! Oiko Inton independently generated mathematical data. CC0-1.0.\n'+name+'\n'+str(len(intervals))+'\n'+'\n'.join(intervals)+'\n'
    destination = root.parent/'crates/inton-core/resources/12edo.scl' if id == '12edo' else root/'tunings'/f'{id}.scl'
    destination.write_text(scl)
    entries.append(dict(source_id='factory:'+id,display_name=name,scl_text=scl,kbm_text=None,category=cat,description=description,listening_hint=listening_hint,source='Independent mathematical construction; formula documented in scripts/generate-factory.py',attribution='Oiko Inton contributors — CC0-1.0',tags=tags,aliases=aliases,favorite=favorite,scl_resource='../crates/inton-core/resources/12edo.scl' if id == '12edo' else f'tunings/{id}.scl'))
def ed(n,p,fav=False):
    name=f'{n} EDO' if p==2 else f'{n} ED{p}'
    if n==13 and p==3:name='Bohlen-Pierce · 13 ED3'
    add(f'{n}ed'+('o' if p==2 else str(p)),name,'Equal Divisions/Octave' if p==2 else 'Equal Divisions/Non-octave','',[f'{1200*math.log2(p)*i/n:.12f}' for i in range(1,n+1)],['equal divisions','octave' if p==2 else 'non-octave','edo' if p==2 else 'tritave' if p==3 else 'wide period'],fav,['bohlen','pierce','bp'] if n==13 and p==3 else [],listening_hint='Explore repeating shapes beyond the octave.' if p!=2 else '')
for n in [5,7,9,10,12,15,17,19,22,24,26,28,31,41,53,72]:ed(n,2,n in [5,7,12,19,22,31])
for n,p in [(13,3),(9,3),(19,3),(13,8),(17,4),(12,4),(7,3),(11,3)]:ed(n,p,n==13 and p==3)
for id,name,ratios,desc,fav in [
 ('ji-major','5-limit major',['9/8','5/4','4/3','3/2','5/3','15/8','2/1'],'Pure major thirds and fifths in a diatonic palette.',True),
 ('ji-minor','5-limit minor',['9/8','6/5','4/3','3/2','8/5','9/5','2/1'],'A minor palette built from small whole-number ratios.',True),
 ('ji-pentatonic','Pure major pentatonic',['9/8','5/4','3/2','5/3','2/1'],'Pure major third and perfect fifth.',False),
 ('septimal','7-limit color',['8/7','7/6','4/3','3/2','12/7','7/4','2/1'],'Includes the 7:4 harmonic seventh.',True),
 ('harmonics8','Harmonics 8–16',[f'{i}/8' for i in range(9,17)],'Successive harmonics folded into the repeating octave.',False),
 ('harmonics16','Harmonics 16–32',[f'{i}/16' for i in range(17,33)],'Closely spaced harmonic-series intervals.',False),
 ('subharmonics','Subharmonics 16–8',[f'16/{i}' for i in range(15,7,-1)],'Descending harmonic numbers give an ascending subharmonic palette.',False),
 ('tritave-ji','Just tritave',['9/7','7/5','5/3','9/5','15/7','7/3','3/1'],'Small whole-number ratios across a tritave.',False),
 ]:add(id,name,'Just Intonation',desc,ratios,['just','ratio','harmonic'],fav)
add('pythagorean','Pythagorean chromatic','Historical / Temperaments','Built from pure 3:2 fifths.', ['256/243','9/8','32/27','81/64','4/3','729/512','3/2','128/81','27/16','16/9','243/128','2/1'],['historical','pythagorean','fifths'])
fifth=1200*math.log2(5**0.25)
notes=sorted([(i*fifth)%1200 for i in range(-5,7) if i!=0])+[1200]
add('quarter-comma','Quarter-comma meantone','Historical / Temperaments','Pure major thirds; the chain includes an uneven closing fifth.',[f'{v:.12f}' for v in notes],['historical','meantone','major thirds'])
for id,p in [('golden', (1+math.sqrt(5))/2),('five-four',1.25)]:
 add('seven-'+id,'7 steps · '+('golden ratio' if id=='golden' else '5:4 period'),'Experimental','',[f'{1200*math.log2(p)*i/7:.12f}' for i in range(1,8)],['experimental','arbitrary period'])
# Original fixed pitch-class offsets, in cents from 12 EDO, ordered C through B.
# Keeping C and A at zero preserves both the octave root and Inton's A440 anchor.
detunings=json.loads((Path(__file__).parent/'electronic-detunings.json').read_text())
for tuning in detunings:
    offsets=tuning['offsets']
    assert len(offsets)==12 and offsets[0]==offsets[9]==0
    cents=[100*i+offsets[i] for i in range(1,12)]+[1200]
    assert all(b>a for a,b in zip([0]+cents,cents))
    strength=max(abs(c) for c in offsets)
    add('detune-'+tuning['id'],tuning['name'],'Electronic/Detuned 12-note',
        f"{tuning['description']} Fixed per-note detuning up to {strength} cents.",
        [f'{c:.12f}' for c in cents],['electronic','techno','synth','detuned','12-note','octave','original'],listening_hint=tuning['listening_hint'])
    entries[-1]['source']='Original Oiko Inton pitch-class offsets, specified in scripts/electronic-detunings.json; not a transcription of any artist or recording.'
# Independently generate Carlos's published rounded step sizes, not an SCL copy.
for title,n,step in [('Alpha',9,78.0),('Beta',11,63.8),('Gamma',20,35.1)]:
    add('carlos-'+title.lower(),f'Carlos {title} · {step:g}-cent steps','Equal Divisions/Non-octave',
        'Published rounded step size from Wendy Carlos. The repeating span is close to a fifth; there is no exact octave.',
        [f'{step*i:.12f}' for i in range(1,n+1)],['carlos',title.lower(),'non-octave','equal steps'])
    entries[-1]['source']='Independent construction from the rounded step sizes published by Wendy Carlos: https://www.wendycarlos.com/resources/pitch.html'
lessons=json.loads((Path(__file__).parent/'first-steps.json').read_text())
for entry in entries:
    entry['favorite']=False
for rank,lesson in enumerate(lessons,1):
    entry=next(e for e in entries if e['source_id']=='factory:'+lesson['id'])
    entry['listening_hint']=lesson['hint']
    entry['favorite']=lesson['id']!='12edo'
    entry['tags']+=['listening-guide',f'lesson:{rank:02}']
(root/'library.json').write_text(json.dumps(entries,ensure_ascii=False,indent=2)+'\n')
print(f'{len(entries)} presets, {sum(e["favorite"] for e in entries)} favorites')
