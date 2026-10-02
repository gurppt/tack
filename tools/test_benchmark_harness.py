import unittest
from run_benchmarks import demand_latency, check_frame_bounds


class BenchmarkEvidenceTests(unittest.TestCase):
    def test_censored_demands_do_not_become_fast_successes(self):
        episodes = [{"delay": value} for value in [None, 0, 50, None, 200]]
        result = demand_latency(episodes, "delay")
        self.assertEqual(result["samples"], 3)
        self.assertEqual(result["censored"], 2)
        self.assertEqual(result["p50_ms"], 50)
        self.assertEqual(result["p95_ms"], 200)
        self.assertEqual(result["p99_ms"], 200)
        missing = demand_latency([{"delay": None}], "delay")
        self.assertIsNone(missing["p50_ms"])
        self.assertEqual(missing["censored"], 1)

    def test_pressure_and_upload_limits_are_enforced_on_recorded_frames(self):
        frame = dict(cpu_cache_bytes=17*1024**2, gpu_resident_bytes=0,
                     pending=16, in_flight=3, upload_bytes=16*1024**2, uploads=8)
        check_frame_bounds([frame], False, 16, 8)
        with self.assertRaisesRegex(RuntimeError, "CPU cache"):
            check_frame_bounds([frame], True, 16, 8)
        for key,value in [("pending",17),("in_flight",4),("uploads",9),("upload_bytes",16*1024**2+1)]:
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, "queue or upload"):
                check_frame_bounds([dict(frame, **{key:value})], False, 16, 8)


if __name__ == "__main__":
    unittest.main()
