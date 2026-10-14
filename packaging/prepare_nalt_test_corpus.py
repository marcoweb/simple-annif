"""Prepare the fixed NALT development sample (not shipped in the portable ZIP).

Requires target/nalt-input/nalt.rdf. Downloads only pinned corpus data;
does not execute notebooks or install dependencies.
"""
import concurrent.futures
import hashlib
import json
import re
import urllib.request
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / 'target/corpus-validation'
ROOT.mkdir(parents=True, exist_ok=True)
NALT = ROOT.parent / 'nalt-input/nalt.rdf'
with NALT.open('rb') as stream:
    nalt_sha256 = hashlib.file_digest(stream, 'sha256').hexdigest()
if nalt_sha256 != '71a3e2e5ca84e5f680e1ebbf3fa7a5892415c58de716334b5add27b54af6d168':
    raise ValueError('NALT RDF differs from the vocabulary used for this fixed test sample')
COMMIT = '7b039e495c4618daae2c8600f3950084c15dfabe'
BASE = f'https://raw.githubusercontent.com/marcoweb/annif-experiments/{COMMIT}/'
TRAIN = 'agriculture-en/raw-files/corpus-training/Corpus_Agricultura_para_Entrenaimento_8000_documentos_TI-RE-PC-RF/'
TEST = 'agriculture-en/raw-files/corpus-test/Corpus_Agricultura_para_Test_1_500_documentos/'

def download(item):
    remote, local = item
    path = ROOT / 'raw' / local
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        with urllib.request.urlopen(BASE + remote, timeout=60) as response:
            path.write_bytes(response.read())
    return {'path': remote, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}

gold_name = 'Gold_indexing_NAL_Agriculture_corpus_entrenaimento_8000.txt'
gold_source = download(('agriculture-en/raw-files/gold-indexing/' + gold_name, gold_name))
items = [(TRAIN + f'D{i}.txt', f'D{i}.txt') for i in range(1, 101)]
items += [(TEST + f'D{i}.txt', f'D{i}.txt') for i in range(8001, 8006)]
with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    sources = [gold_source, *pool.map(download, items)]
print('Downloaded 100 training and 5 held-out documents', flush=True)
labels = defaultdict(set)
skos = '{http://www.w3.org/2004/02/skos/core#}'
rdf = '{http://www.w3.org/1999/02/22-rdf-syntax-ns#}'
xml = '{http://www.w3.org/XML/1998/namespace}'
for _, element in ET.iterparse(NALT, events=('end',)):
    uri = element.get(rdf + 'about')
    if uri:
        for child in element:
            if child.tag in (skos + 'prefLabel', skos + 'altLabel') and child.get(xml + 'lang') == 'en' and child.text:
                labels[child.text.strip().casefold()].add(uri)
        element.clear()
gold = {}
for line in (ROOT / 'raw' / gold_name).read_text(encoding='utf-8-sig').splitlines():
    identity, sep, terms = line.partition(':')
    if sep:
        gold[identity.strip()] = [t.strip() for t in terms.split(',') if t.strip()]
unmatched = defaultdict(int)
ambiguous = defaultdict(int)
count = 0
assigned = set()
for i in list(range(1, 101)) + list(range(8001, 8006)):
    identity = f'D{i}'
    text = (ROOT / 'raw' / f'{identity}.txt').read_text(encoding='utf-8-sig')
    title = re.search(r'#ITI#(.*?)#FTI#', text, re.S)
    abstract = re.search(r'#IRE#(.*?)#FRE#', text, re.S)
    if not title or not abstract:
        raise ValueError(f'Missing title/abstract: {identity}')
    destination = ROOT / ('train' if i <= 100 else 'test')
    destination.mkdir(exist_ok=True)
    if i <= 100:
        uris = set()
        for term in gold[identity]:
            matches = labels.get(term.casefold(), set())
            if len(matches) == 1:
                uris.update(matches)
            elif not matches:
                unmatched[term] += 1
            else:
                ambiguous[term] += 1
        if not uris:
            raise ValueError(f'No mapped subjects: {identity}')
        assigned.update(uris)
        (destination / f'{identity}.key').write_text('\n'.join(f'<{u}>' for u in sorted(uris)) + '\n', encoding='utf-8')
        count += 1
    (destination / f'{identity}.txt').write_text(title[1].strip() + '\n\n' + abstract[1].strip(), encoding='utf-8')
report = {'repository': 'https://github.com/marcoweb/annif-experiments', 'commit': COMMIT, 'nalt_sha256': nalt_sha256,
          'training_documents': count, 'held_out_documents': 5, 'mapped_subjects': len(assigned),
          'input_text': 'title and abstract only; keywords and references excluded',
          'mapping': 'English NALT prefLabel and altLabel; case-insensitive exact matches; ambiguous and unmatched excluded',
          'unmatched': dict(unmatched), 'ambiguous': dict(ambiguous), 'sources': sources}
(ROOT / 'preparation.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k not in ('sources','unmatched','ambiguous')}, ensure_ascii=False), flush=True)
print(f'Unmatched terms: {len(unmatched)}; ambiguous: {len(ambiguous)}', flush=True)
