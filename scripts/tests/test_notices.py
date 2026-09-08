import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import notices


class NoticeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / 'output'
        self.product = SimpleNamespace(key='inton', package='plugin')
        for name in ['plugins/inton/LICENSE', 'plugins/inton/THIRD_PARTY.md',
                     'plugins/inton/vendor/mts-esp/LICENSE', 'vendor/tuning-library/LICENSE.md',
                     'crates/oiko-ui/assets/Ubuntu-Font-Licence-1.0.txt']:
            self.write(name, name)
        self.write('licenses/upstream.json', '{}')
        self.write('registry/library/LICENSE', 'upstream text')
        self.write('registry/library/fonts/OFL.txt', 'font text')
        self.metadata = {'packages': [], 'resolve': {'nodes': []}}
        for name in ['plugin', 'library', 'dev-only', 'build-tool']:
            self.metadata['packages'].append({
                'id': name, 'name': name, 'version': '1.0', 'license': 'MIT',
                'repository': 'https://example.test/upstream',
                'manifest_path': str(self.root / 'registry' / name / 'Cargo.toml'),
                'source': None if name == 'plugin' else 'registry+test',
            })
            self.metadata['resolve']['nodes'].append({'id': name, 'deps': []})
        self.metadata['resolve']['nodes'][0]['deps'] = [
            {'pkg': name, 'dep_kinds': [{'kind': kind}]} for name, kind in
            [('library', None), ('dev-only', 'dev'), ('build-tool', 'build')]]
        self.write('registry/build-tool/LICENSE', 'build-tool text')
        mocked = patch.object(notices, 'ROOT', self.root)
        mocked.start()
        self.addCleanup(mocked.stop)

    def write(self, path, text):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text)
        return destination

    def test_collects_transitive_notices_fonts_and_native_licenses_without_dev_dependencies(self):
        inventory = notices.collect(self.product, self.output, self.metadata)
        self.assertEqual({p['name'] for p in inventory}, {'library', 'build-tool'})
        for source, target in [
            ('registry/library/LICENSE', 'licenses/library-1.0/LICENSE'),
            ('registry/library/fonts/OFL.txt', 'licenses/library-1.0/fonts/OFL.txt'),
            ('vendor/tuning-library/LICENSE.md', 'licenses/tuning-library/LICENSE.md'),
            ('plugins/inton/vendor/mts-esp/LICENSE', 'licenses/mts-esp/LICENSE'),
        ]:
            self.assertEqual((self.root / source).read_bytes(), (self.output / target).read_bytes())
        self.assertNotIn(str(self.root), (self.output / 'dependency-licenses.json').read_text())

    @unittest.skipIf(sys.platform == "win32", "Symlink creation needs extra Windows privileges")
    def test_vendored_dependency_is_included_through_a_symlinked_workspace(self):
        vendor = self.write('vendor/library/Cargo.toml', '')
        self.write('vendor/library/LICENSE', 'vendored notice')
        package = self.metadata['packages'][1]
        package['source'] = None
        package['manifest_path'] = str(vendor)
        alias = self.root / 'workspace-alias'
        alias.symlink_to(self.root, target_is_directory=True)
        with patch.object(notices, 'ROOT', alias):
            inventory = notices.collect(self.product, self.output, self.metadata)
        self.assertIn('library', {p['name'] for p in inventory})

    def test_shared_dev_and_normal_dependency_is_included(self):
        self.metadata['resolve']['nodes'][1]['deps'] = [
            {'pkg': 'dev-only', 'dep_kinds': [{'kind': None}]}]
        self.write('registry/dev-only/LICENSE', 'normal transitive dependency')
        inventory = notices.collect(self.product, self.output, self.metadata)
        self.assertIn('dev-only', {p['name'] for p in inventory})

    def test_missing_notice_requires_reviewed_source(self):
        (self.root / 'registry/build-tool/LICENSE').unlink()
        with self.assertRaisesRegex(ValueError, 'Missing upstream license notices for build-tool-1.0'):
            notices.collect(self.product, self.output, self.metadata)

    def test_reviewed_source_is_copied_and_checksum_changes_are_rejected(self):
        (self.root / 'registry/build-tool/LICENSE').unlink()
        source = self.write('licenses/upstream/build-tool/LICENSE', 'reviewed notice')
        self.write('licenses/upstream.json', json.dumps({'build-tool-1.0': [{
            'path': 'licenses/upstream/build-tool/LICENSE',
            'url': 'https://example.test/exact-commit/LICENSE',
            'sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
        }]}))
        notices.collect(self.product, self.output, self.metadata)
        self.assertEqual(source.read_bytes(), (self.output / 'licenses/build-tool-1.0/LICENSE').read_bytes())
        source.write_text('changed')
        with self.assertRaisesRegex(ValueError, 'Changed upstream notice'):
            notices.collect(self.product, self.root / 'other-output', self.metadata)


if __name__ == '__main__':
    unittest.main()
