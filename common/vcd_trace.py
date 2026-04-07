import os
from vcdvcd import VCDVCD
import sys
import json
import tqdm
import data_types
import numpy as np

class vcdTrace:

    def __init__(self, tracePath, clkPath, cache_values=True, signals=None):
        #print("Loading vcdTrace..")
        self.vcd = VCDVCD(tracePath)
        #print("Loading vcdTrace done")
        #print("Clock path", self.vcd[clkPath])
        if len(self.vcd[clkPath].tv) < 3:
            self.clk_freq = self.vcd.endtime
        else:
            self.clk_freq = self.vcd[clkPath].tv[2][0]- self.vcd[clkPath].tv[0][0]
        self.tracePath = tracePath
        self.value_cache_str= {}
        self.replace_dontcare_with_zeros = True
        if cache_values is True:
            self.cache_values()
                
        #print("clk freq", self.clk_freq,self.vcd[clkPath].tv )
    def cache_values(self, signals=None):
        #self.value_cache_str= {}
        cache_signals = signals
        if cache_signals is None:
            cache_signals = self.vcd.references_to_ids.keys()
        for signal in cache_signals:
            self.value_cache_str[signal] = {}
            for cycle in range(self.getNumCycles()):
                timestep = self.getTimeFromCycle(cycle)
                #print("Timestep", timestep)
                val = self.vcd[signal][timestep]
                #print("Signal", val, "type", type(val))
                if val == 'x' and self.replace_dontcare_with_zeros:
                    #Replace x (dontcare) val by 0
                    val = 0
                else:
                    val = int(val, 2)
                self.value_cache_str[signal][cycle] = str(val)
    
    def get_signal_value_at_cycle_str(self,signal: str, cycle: int) -> str:
        if signal in self.value_cache_str and cycle in self.value_cache_str[signal]:
            return self.value_cache_str[signal][cycle]
        else:
            val = self.get_signal_value_at_cycle(signal, cycle)
            if signal not in self.value_cache_str:
                self.value_cache_str[signal] = {}
            self.value_cache_str[signal][cycle] = str(val)
            return self.value_cache_str[signal][cycle]
        
            
    def getTimeFromCycle(self, cycle):
        # Return the negative edge
        return int((cycle+0.5) * self.clk_freq)

    def getCycleFromTime(self, time):
        return int(time /self.clk_freq) #Round Down

    def get_signal_value_at_cycle(self, signal, cycle):
        timestep = self.getTimeFromCycle(cycle)
        #print("Timestep", timestep)
        val = self.vcd[signal][timestep]
        #print("Signal", val, "type", type(val))
        val = int(val, 2)
        #print("Signal", val, "type", type(val))
        return val

    def getNumCycles(self):
        #print("Endtime", self.vcd.endtime)
        return self.getCycleFromTime(self.vcd.endtime)
    
    def timeout(self):
        return self.vcd.endtime
    
    def get_signals_and_length(self):
        ret_list = []
        for signal in self.vcd.references_to_ids.keys():
            val = self.vcd[signal][0]
            signal_length = len(val)
            signal_info = data_types.SignalInfo(name=signal, bit_length=signal_length)
            ret_list.append(signal_info)
        return ret_list

    def get_signal_to_length_dict(self):
        signals = self.vcd.references_to_ids.keys()
        signal_sizes = {}
        for signal in signals:
            signal_sizes[signal] = int(self.vcd.data[self.vcd.references_to_ids[signal]].size)
        return signal_sizes

    def get_waveform_matrix(self, signal_list, search_in_cycles=None):
        if search_in_cycles is None:
            num_cycles = self.getNumCycles()
            search_in_cycles = range(num_cycles)
        
        # Precompute values for all signals across cycles using a vectorized approach
        waveform_matrix = np.array([
            [self.get_signal_value_at_cycle(signal, cycle) for signal in signal_list]
            for cycle in search_in_cycles
        ])
        
        return waveform_matrix
        """
            num_cycles = self.getNumCycles()
            num_signals = len(signal_list)
            waveform_matrix = np.zeros((num_cycles, num_signals))
            for i in range(num_cycles):
                waveform_matrix[i, :] = [self.get_signal_value_at_cycle(signal, i) for signal in signal_list]
            return waveform_matrix
        """

if __name__ == "__main__":
    v = vcdTrace(sys.argv[1], sys.argv[2])
    print("\n".join(v.vcd.references_to_ids.keys()))
   
