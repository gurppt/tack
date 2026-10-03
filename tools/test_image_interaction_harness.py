import unittest
from run_image_interaction import distribution


class DistributionTest(unittest.TestCase):
    def test_empty_and_native_tail_are_not_hidden(self):
        self.assertIsNone(distribution([])['p99_ms'])
        sample = distribution([1.] * 100 + [70.])
        self.assertEqual(sample['p99_ms'], 1.)
        self.assertEqual(sample['max_ms'], 70.)
        self.assertEqual(sample['samples'], 101)
