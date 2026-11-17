"""Verify the same-version Macroquad atlas retirement patch and source closure."""
import hashlib
import json
import pathlib


def main():
    root = pathlib.Path(__file__).resolve().parents[1]
    vendor = root / 'vendor/macroquad'
    record = json.loads((vendor / 'source-provenance.json').read_text())
    assert record['package'] == 'macroquad' and record['version'] == '0.4.16'
    assert set(record['patches']) == {'src/text/atlas.rs', 'src/texture.rs'}
    expected = {**record['files'], **record['patches'], **record['addedFiles']}
    for name, info in expected.items():
        path = vendor / name
        assert path.resolve().is_relative_to(vendor.resolve()), name
        data = path.read_bytes()
        assert len(data) == info['bytes'], name
        assert hashlib.sha256(data).hexdigest() == info['sha256'], name
    actual = {p.relative_to(vendor).as_posix() for p in vendor.rglob('*') if p.is_file()}
    assert actual == set(expected) | {'source-provenance.json'}
    assert len(record['registryChecksum']) == 64
    print(f"Macroquad 0.4.16: {len(record['files'])} upstream files; two patched Rust files; licenses/provenance intact")


if __name__ == '__main__':
    main()
