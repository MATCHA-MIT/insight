import collections


SignalInfo = collections.namedtuple("SignalInfo", ["name", "bit_length"])


CounterExample = collections.namedtuple('CounterExample', ['waveform', 'search_in_cycles', 'waveform_path', 'bin_path'])   

