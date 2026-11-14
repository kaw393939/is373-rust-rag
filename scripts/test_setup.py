"""Configuration tests never contact providers or contain real credentials."""
import os
from pathlib import Path
import tempfile
import unittest
from config import load, save

class ConfigurationTests(unittest.TestCase):
    def test_round_trip_private_nonexecuting_values(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'.env.local'
            value={'CHAT_PROVIDER':'groq','TEST_VALUE':'$(this-must-not-run) `nor-this`','GROQ_API_KEY':'test-only-placeholder'}
            save(value,path)
            self.assertEqual(load(path),value)
            if os.name=='posix':self.assertEqual(path.stat().st_mode & 0o777,0o600)
    def test_rejects_invalid_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'.env.local';path.write_text('export GROQ_API_KEY=wrong-format\n')
            with self.assertRaises(ValueError):load(path)
if __name__=='__main__':unittest.main()
