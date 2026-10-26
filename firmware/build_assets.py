#!/usr/bin/env python3
"""Build a SPIFFS asset image; never reads the repo's real network.json.
Usage: python3 build_assets.py --target esp32 --source /your/staged-assets
The source must explicitly contain config/network.json to enable Wi-Fi.
"""
import argparse
import os
from pathlib import Path
import subprocess
import sys

ROOT=Path(__file__).resolve().parent

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target',choices=['esp32','esp32-bare','esp32s3'],required=True)
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--output',type=Path)
    parser.add_argument('--idf-path',type=Path,default=Path(os.environ.get('IDF_PATH',ROOT/'.embuild/espressif/esp-idf/v5.2.1')))
    args=parser.parse_args()
    if not args.source.is_dir(): parser.error('source directory does not exist')
    table=ROOT/('partitions_16MB.csv' if args.target=='esp32s3' else 'partitions.csv')
    size=offset=None
    for line in table.read_text().splitlines():
        parts=[p.strip() for p in line.split(',')]
        if len(parts)>=5 and parts[2]=='spiffs': offset=int(parts[3],0);size=int(parts[4],0)
    if size is None: parser.error('SPIFFS partition absent')
    tool=args.idf_path/'components/spiffs/spiffsgen.py'
    if not tool.exists(): parser.error('ESP-IDF spiffsgen.py not found; supply --idf-path')
    for path in args.source.rglob('*'):
        if path.is_symlink(): parser.error('symlink assets are not supported')
        if path.is_file() and len(('/'+path.relative_to(args.source).as_posix()).encode())>=32:
            parser.error(f'SPIFFS filename exceeds 31 bytes: {path.relative_to(args.source)}')
    output=args.output or ROOT/'target'/f'assets-{args.target}.bin'
    output.parent.mkdir(parents=True,exist_ok=True)
    subprocess.run([sys.executable,str(tool),str(size),str(args.source),str(output)],check=True)
    print(f'Created {output} ({output.stat().st_size} bytes), partition offset {offset:#x}; no hardware written.')

if __name__=='__main__':main()
