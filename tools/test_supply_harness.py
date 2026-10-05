import unittest
from run_supply import summarize, percentile


class SupplyHarness(unittest.TestCase):
    def test_unfinished_jump_remains_censored_and_raw_timings_are_distinct(self):
        def frame(t, visible, image, resolved):
            f = {k: 1. for k in ('query_ms', 'scene_ms', 'supply_ms', 'encode_ms',
                                 'submit_ms', 'poll_ms', 'acquire_ms', 'present_ms', 'callback_ms')}
            f.update(elapsed_ms=t, visible=visible, recognizable=image, quality_resolved=resolved,
                     detailed=resolved, gpu_bytes=1024)
            return f
        data = {'frames': [frame(1000, 5, 0, 0), frame(1100, 5, 4, 1),
                           frame(3100, 5, 0, 0)], 'ordinary_view_80_percent_ms': 1100,
                'cpu_payload_peak': 1024, 'peak_pending': 4, 'peak_queued': 3,
                'cpu_evictions': 1, 'discarded': 1, 'reprioritized': 3,
                'source_bytes_before_detail': 42, 'container_bytes': 80,
                'metadata_load_ms': 2., 'gpu_samples': [{'pass_ms': .1}]}
        s = summarize(data)
        self.assertEqual(s['episodes'][0]['useful_delay_ms'], 100)
        self.assertIsNone(s['episodes'][0]['quality_delay_ms'])
        self.assertIsNone(s['episodes'][1]['useful_delay_ms'])
        self.assertEqual(s['gpu_ms_p99'], .1)
        self.assertIsNone(percentile([], .99))


if __name__ == '__main__':
    unittest.main()
