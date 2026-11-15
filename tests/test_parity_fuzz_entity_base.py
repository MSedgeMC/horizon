import random
import unittest

from tools import parity_fuzz_scenarios as families


class EntityBaseFamilyTests(unittest.TestCase):
    def test_one_scenario_per_kind_and_variant_with_the_sweep_steps(self):
        families.KNOWN['entity'] = {'pig': {}, 'arrow': {}, 'armor_stand': {}}
        kinds = sorted(families.KNOWN['entity'])
        scenarios = [families.entity_base(random.Random(seed), seed) for seed in range(2 * len(kinds))]
        self.assertEqual(len(scenarios), 6)
        self.assertEqual([s['steps'][0]['type'] for s in scenarios], kinds + kinds)
        self.assertEqual(scenarios[0]['steps'][0]['nbt'], families.BASE_TAGS['still'])
        self.assertEqual(scenarios[len(kinds)]['steps'][0]['nbt'], families.BASE_TAGS['fire'])
        self.assertEqual([step['k'] for step in scenarios[0]['steps']],
                         ['summon', 'read_entity', 'tick', 'read_entity', 'tick', 'read_entity', 'reload',
                          'read_entity'])
        self.assertEqual([step['n'] for step in scenarios[0]['steps'] if step['k'] == 'tick'], [1, 19])


if __name__ == '__main__':
    unittest.main()
