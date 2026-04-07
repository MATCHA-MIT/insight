import csv
import logging
import random
import yaml
from .instruction import Instruction
from .instruction_type import InstructionType
from .utils import int_to_bits, get_relative_path, value_to_binary_string
from . import utils
import enum
from . import program
import copy
from collections import Counter
import typing

class OperandMutationProbabilities(enum.Enum):
    ADD = "ADD"
    SUBSTRACT = "SUBSTRACT"
    CHOSE_FROM_CONSTANT = "CHOSE_FROM_CONSTANT"
    FLIP_BIT = "FLIP_BIT"
    RANDOM_VALUE = "RANDOM_VALUE"


# Define the probability dictionary
probability_dict = {
    OperandMutationProbabilities.ADD.value: 0.2,
    OperandMutationProbabilities.SUBSTRACT.value: 0.2,
    OperandMutationProbabilities.CHOSE_FROM_CONSTANT.value: 0.2,
    OperandMutationProbabilities.FLIP_BIT.value: 0.2,
    OperandMutationProbabilities.RANDOM_VALUE.value: 0.2
}

class OperandMutator:
    def __init__(self, constants):
        self.constants = list(set([0,1, 0x80000000, 0x80000, 0x10000]+constants))

    def mutate_operand(self, operand_bits, bit_length, new_value=None):
        if new_value is not None:
            return int(value_to_binary_string(new_value, bit_length, clamp=True), 2)
            
        operand = int(operand_bits, 2)
        # Choose an operation based on the probability distribution
        operation = random.choices(
            list(probability_dict.keys()),
            list(probability_dict.values()),
            k=1
        )[0]
        
        if operation == OperandMutationProbabilities.ADD.value:
            new_operand = self.add_random_integer(operand, max_value=(2 ** (bit_length))-operand)
        elif operation == OperandMutationProbabilities.SUBSTRACT.value:
            new_operand = self.subtract_random_integer(operand, max_value=(2 ** (bit_length)))
        elif operation == OperandMutationProbabilities.CHOSE_FROM_CONSTANT.value:
            new_operand = self.choose_from_constant()
            if new_operand > (2 ** (bit_length)):
                new_operand = operand
        elif operation == OperandMutationProbabilities.FLIP_BIT.value:
            new_operand = self.flip_random_bit(operand, bit_length)
        elif operation == OperandMutationProbabilities.RANDOM_VALUE.value:
            new_operand = random.randint(0, (2 ** (bit_length)))
            
        return int(value_to_binary_string(new_operand, bit_length, clamp=True), 2)
        

    def _get_random_integer(self, max_value):
        """Get a random integer, with 10% chance of using a non-zero constant."""
        if random.random() < 0.1:
            non_zero_constants = [x for x in self.constants if x != 0]
            if non_zero_constants:
                return random.choice(non_zero_constants)
        
        # Ensure max_value is at least 1 for randint
        if max_value < 1:
            max_value = 1
        return random.randint(1, max_value)

    def add_random_integer(self, operand, max_value):
        random_int = self._get_random_integer(max_value)
        return operand + random_int

    def subtract_random_integer(self, operand, max_value):
        random_int = self._get_random_integer(max_value)
        return operand - random_int

    def choose_from_constant(self):
        return random.choice(self.constants)

    def flip_random_bit(self, operand, bit_length):
        #bit_length = operand.bit_length()
        #if bit_length == 0:
        #    return operand  # If operand is 0, no bits to flip
        bit_to_flip = random.randint(0, bit_length - 1)
        return operand ^ (1 << bit_to_flip)

logger = logging.getLogger('mutator')

class Mutator():
    def __init__(self, bytecode, mutation_steps: int, constants = None, interesting_instructions:  typing.Optional[list[int]] = None, check_mutations_valid: bool = True,
                 max_program_length: int = 20000):
        #self._instruction_dict = instruction_dict 
        self.bytecode = bytecode
        #for instr in self._instruction_dict.keys():
        #    self.bytecode += instr.to_bytes(4, "little")
        self.program = program.Program(self.bytecode)
        self._mutation_steps = mutation_steps
        self._arg_lut = utils._load_arg_lut()
        self._instruction_types = utils._load_instruction_types()
        logger.debug(f"instructions {self._instruction_types}")
        self.constants = constants
        if self.constants is None:
            self.constants = []
        self.operand_mutator = OperandMutator(self.constants)
        self.choices = {
            "opcode": 0.1,
            "operands":0.45,
            "new_instruction": 0.1,
            "delete_instruction": 0.05,
            "change_source_operand_and_last_write": 0.2,
            "switch_instructions": 0.1
        }
        if interesting_instructions is None:
            self.interesting_instructions = range(0, len(self.program.instructions))
        else:
            self.interesting_instructions = interesting_instructions
        self.check_mutations_valid = check_mutations_valid
        self.max_program_length = max_program_length
        
    def mutate_operand(self, operand_bits, bit_length, new_value=None): 
        """
        Return new opcode
        """
        return self.operand_mutator.mutate_operand(operand_bits, bit_length, new_value)

    def do_one_mutation(self, current_program, current_interesting_instructions, choice_arg=None, mutate_field=None, arg_mutate_new_value=None) -> tuple[program.Program, list[int]]:
        """
        Perform one mutation on the bytecode.
        :param choice_arg: A tuple of (instruction index, mutation choice) or None to choose randomly.
        :param mutate_field: The field to mutate, if applicable.
        :param arg_mutate_new_value: The new value for the operand if applicable.
        :return: The mutated program.
        """
        if choice_arg is None:
            if current_interesting_instructions:
                mutated_instr_idx = random.choice(current_interesting_instructions)
            else:
                mutated_instr_idx = random.choice(range(0, current_program.get_length()))
            mutation_choice = None
        else:
            mutated_instr_idx, mutation_choice = choice_arg
        try:
            mutated_instr = current_program.instructions[mutated_instr_idx]
        except IndexError as e:
            logger.error(f"Invalid instruction index {mutated_instr_idx} for program of length {current_program.get_length()}")
            logger.error(f"Current interesting instructions: {current_interesting_instructions}")
            raise e
        if mutated_instr._instr_details is None and current_program.get_length() == 1:
            logger.warning(f"Cannot mutate an invalid instruction {mutated_instr._instruction_bytes}, skipping this mutation")
            return current_program, current_interesting_instructions
        if  len(current_interesting_instructions) < 2 and mutation_choice == "switch_instructions":
            logger.warning(f"Not enough interesting instruction to switch_instructions, skipping this mutation")
            return current_program, current_interesting_instructions
        
        potential_choices = copy.deepcopy(self.choices)

        if mutated_instr._instr_details is None:
            potential_choices.pop("opcode")
            potential_choices.pop("operands")
        if current_program.get_length() <= 1 or len(current_interesting_instructions) <= 1:
            potential_choices.pop("delete_instruction")
        if current_program.get_length() < 2 or len(current_interesting_instructions) < 2:
            potential_choices.pop("switch_instructions")
        if current_program.get_length() >= self.max_program_length:
            potential_choices.pop("new_instruction")
        if mutation_choice is None or mutation_choice not in potential_choices:
            mutation_choice = random.choices(list(potential_choices.keys()), weights=potential_choices.values())[0]    

        logger.debug(f"Doing mutation {mutation_choice}")
        # print(f"Doing mutation {mutation_choice} at index {mutated_instr_idx} to {mutate_field} with new value {arg_mutate_new_value}")
        match mutation_choice:
            case "opcode":
                logger.debug(f"Mutating opcode")
                logger.debug(f"Mutating instruction \"{mutated_instr._instr_details.get_mnemonics()}\"")
                
                if random.random() < 0.5:
                    # Mutate the instruction into a random one
                    new_instr = random.choice(list(self._instruction_types))
                    try:
                        mutated_instr = self._generate_instruction(new_instr, mutated_instr)
                        # logger.debug(f"Mutated instruction \"{mutated_instr.instr_details.get_mnemonics()}\"")
                    except Exception as e:
                        logger.error(f"Error while generating instruction: {e}")
                        raise e
                    
                else:
                    # Mutate the instruction into one with exactly the same operands
                    new_instr = self._get_instruction_with_same_operands(mutated_instr)
                    try:
                        logger.debug(f"Type of mutated_instr: {type(mutated_instr)}")
                        mutated_instr = self._substitute_opcode(new_instr, mutated_instr._instr_details.get_var_operands())
                        # logger.debug(f"Mutated instruction \"{mutated_instr.instr_details.get_mnemonics()}\"")
                    except Exception as e:
                        logger.error(f"Error while generating instruction: {e}")
                        raise e
                    
                logger.debug(f"Mutated instr {hex(mutated_instr)}")
                if self.check_mutations_valid and not utils.check_instr_valid(mutated_instr):
                    logger.warning(f"Mutated instruction {hex(mutated_instr)} did not result in valid instruction, skipping this mutation")
                    return current_program, current_interesting_instructions
                current_program.instructions[mutated_instr_idx] = Instruction(mutated_instr)
            case "operands":
                # Mutate one operand only
                logger.debug("Mutating operand")  
                #With a certain chance, mutate operand                 
                # See https://www.researchgate.net/figure/Six-basic-instruction-formats-of-the-RISC-V-instruction-set-3-RISC-V-processor-core_fig1_360788701
                # If we modify funct3 or funct7, then we either modify exactly those fields
                # Or in the case of an I-type/S-type funct7 is modification of imm10
                # In the case of an B/U/J type, funct3 in modificaiton of imm
            #    chosen_operand = random.choice(["funct3", "funct7"])

                var_operands = mutated_instr._instr_details.get_var_operands()
                
                if random.random() < 0.2:
                    var_operands = var_operands + ["funct3", "funct7"]
                #print("Mutating with operands", var_operands)
                if var_operands:
                    if mutate_field is not None and mutate_field in var_operands:
                        chosen_operand = mutate_field
                    else:
                        chosen_operand = random.choice(var_operands)
                    
                    logger.debug(f"Mutating operand {chosen_operand} in instruction {mutated_instr._instr_details.get_mnemonics()}")
                    beg, end = self._arg_lut[chosen_operand]
                    if beg == end:
                        logger.critical(f"For {mutated_instr._instr_details} beg == end")
                        # print(f"For {mutated_instr._instr_details} beg == end")
                        return current_program, current_interesting_instructions
                    
                    operand_bits = int_to_bits(mutated_instr._instruction_integer, 32)[-(int(beg)+1):-int(end)]
                    
                    logger.debug(f"instr to bits {int_to_bits(mutated_instr._instruction_integer, 32)}, beg {beg}, end {end}, operand_bits {operand_bits}")
                    
                    operand = int(operand_bits,2)
                    mutated_operand = self.mutate_operand(operand_bits, bit_length=(int(beg)-int(end)+1), new_value=arg_mutate_new_value)
                    
                    after_mutation_instr_integer = utils.set_operand(mutated_instr._instruction_integer,mutated_operand_val=mutated_operand,beg=beg,end=end)
                    
                    logger.info(f"Mutating operand {chosen_operand} from value {operand} to value {mutated_operand}")
                    logger.debug(f"Instruction after mutation (hex): {hex(after_mutation_instr_integer)} before {mutated_instr._instruction_bytes}")

                    if chosen_operand in ["rs1", "rs2"]:
                        if random.random() < 0.5:
                            #new_program = program.Program(self.bytecode)

                            #print("Changing source operand for instruction %s", mutated_instr._instr_details.get_mnemonics())
                            if logger.isEnabledFor(logging.DEBUG):
                                pass#logger.debug("Before change %s", current_program.code_as_hexstring())

                            current_program.change_source_operand_and_all_prior_writes(mutated_instr_idx, mutated_operand)
                            if logger.isEnabledFor(logging.DEBUG):
                                pass#logger.debug("New program bytecode %s", current_program.code_as_hexstring())
                            #continue
                        else:
                            #TODO: Beautify this code. Also, recompute the write map!
                            current_program.instructions[mutated_instr_idx] = Instruction(after_mutation_instr_integer)
                    else:
                        current_program.instructions[mutated_instr_idx] = Instruction(after_mutation_instr_integer)
                    if self.check_mutations_valid and not utils.check_instr_valid(after_mutation_instr_integer):
                        raise Exception(f"Mutated instruction {hex(after_mutation_instr_integer)} did not result in valid instruction, but why? This mutation should always be valid?")
                    #logger.info(f"mutated_sequence is now {mutated_sequence}")
                else: return current_program, current_interesting_instructions
                if mutated_instr._instruction_integer == after_mutation_instr_integer:
                    logger.warning("Mutation did not change the instruction")
                    return current_program, current_interesting_instructions
                #else: 
                    #logging.info("Deleting {instr.instrunction_bytes}")
                #    del mutated_sequence[instr.instruction_bytes]
            case "change_source_operand_and_last_write":
                print("Changing source operand and last write")
                #print("Current program instructions before", [str(x) for x in current_program.instructions])
                print("Mutation step", self._mutation_steps)
                new_value = self.mutate_operand("0", 5, arg_mutate_new_value)
                print(f"Mutating source operand and last write to {hex(new_value)}")
                current_program.change_source_operand_and_all_prior_writes(mutated_instr_idx, new_value)
                #print("Current program instructions after", [str(x) for x in current_program.instructions])
            case "new_instruction":
                logger.info("Adding new instruction")
                #new_instr_index = random.choice([mutated_sequence[key].get("index", float('-inf')) for key in mutated_sequence.keys()])
                #new_instr_index = random.choice(range(0, current_program.get_length()))
                choose_from = []
                for i in current_interesting_instructions:
                    choose_from.append(i)
                    if i+1 < current_program.get_length():
                        choose_from.append(i+1)
                    if i+2 < current_program.get_length():
                        choose_from.append(i+2)
                    if i-1 >= 0:
                        choose_from.append(i-1)
                    if i-2 >= 0:
                        choose_from.append(i-2)
                if len(choose_from) == 0:
                    new_instr_index = random.choice(range(0, current_program.get_length()))
                else:
                    new_instr_index = random.choice(choose_from)
                new_instr = random.choice(list(self._instruction_types))
                try: 
                    mutated_instr = self._generate_instruction(new_instr)
                except Exception as e:
                    raise Exception(f"Error while generating instruction: {e}")
                if self.check_mutations_valid and not utils.check_instr_valid(mutated_instr):
                    logger.warning(f"Added instruction {hex(mutated_instr)} did not result in valid instruction, skipping this mutation")
                    return current_program, current_interesting_instructions
                current_program.instructions.insert(new_instr_index, Instruction(mutated_instr))
                current_interesting_instructions = [i+1 if i >= new_instr_index else i for i in current_interesting_instructions]\
               
                #logger.debug(f"Added instruction \"{new_instr.get_mnemonics()}\" at index {new_instr_index}")             
            case "delete_instruction":
                logger.info("Deleting instruction")
                if current_program.get_length() == 1:
                    raise Exception("Cannot delete the only instruction in the sequence")
                instr_to_delete_idx = random.choice(current_interesting_instructions)
                current_program.delete_instruction(instr_to_delete_idx)
                current_interesting_instructions.remove(instr_to_delete_idx)
                current_interesting_instructions = [i-1 if i >= instr_to_delete_idx else i for i in current_interesting_instructions]
                logger.info(f"Deleted instruction at index {instr_to_delete_idx}")
                #instruction_to_delete = random.choice(list(mutated_sequence.keys()))
                #logger.debug(f"Deleting instruction at index {mutated_sequence[instruction_to_delete].get('index', float('-inf'))}")
                #del mutated_sequence[instruction_to_delete]

            case "switch_instructions":
                logger.info("Switching two instructions")
                if current_program.get_length() < 2 or len(current_interesting_instructions) < 2:
                    raise Exception("Cannot switch instructions in a sequence with less than 2 instructions")
                idx1, idx2 = random.sample(current_interesting_instructions, 2)
                current_program.switch_instructions(idx1, idx2)
                logger.info(f"Switched instructions at index {idx1} and {idx2}")
            case _:
                logger.critical("Invalid choice")
        return current_program, current_interesting_instructions
    
    def do_list_of_mutations(self, mutations: list[tuple[int,str], str, int]) -> program.Program:
        """
        Perform a list of mutations on the bytecode.
        :param mutations: A list of tuples, each containing (instruction index, mutation choice), mutate_field, and arg_mutate_new_value.
        :return: The mutated program.
        """
        current_program = copy.deepcopy(self.program)
        current_interesting_instructions = copy.deepcopy(self.interesting_instructions)
        
        for choice_arg, mutate_field, arg_mutate_new_value in mutations:
            current_program, current_interesting_instructions = self.do_one_mutation(
                current_program, 
                current_interesting_instructions, 
                choice_arg,
                mutate_field,
                arg_mutate_new_value
            )
        
        return current_program
    
    # TODO: make the probabilities of mutating the instruction and the operands configurable
    def mutate_random(self) -> program.Program:
        current_program = copy.deepcopy(self.program)
        current_interesting_instructions = copy.deepcopy(self.interesting_instructions)
        #print(f"Current program before mutation: {[str(x) for x in current_program.instructions]}")
        for i in range(self._mutation_steps):
            current_program, current_interesting_instructions = self.do_one_mutation(
                current_program, 
                current_interesting_instructions, 
                None,
                None,
                None
            )
        return current_program
        #logging.info(f"Mutated sequence {mutated_sequence}")
        # Return the mutated instruction sequence, ordered by the index of the instruction
        #return sorted(mutated_sequence.keys(), key=lambda key: mutated_sequence[key].get("index", float('-inf')))
                
    def _build_instruction_with_operands(self, instr_details: InstructionType, operands: list[str], old_instruction: Instruction = None) -> int:
        """
        Build an instruction with the given operands.
        """
        match = instr_details.get_match()
        instr = match
        
        for operand in operands:
            operand_value = None
            beg, end = self._arg_lut[operand]
            field_width = int(beg) - int(end) + 1
            # Try to reuse operand from old instruction if available
            if old_instruction is not None and random.randint(0, 4) != 0:  # 80% chance to reuse
                operand_value = old_instruction.get_field_if_exists_as_int(operand)
            
            # Generate random operand value if not reused
            if operand_value is None:
                if random.random() < 0.1 and self.constants:
                    operand_value = random.choice(self.constants)
                    operand_value = min(operand_value, (2 ** field_width) - 1)  # Clamp to field size
                else:
                    operand_value = random.randint(0, (2 ** field_width) - 1)

            # Mask to field width and shift into position
            field_mask = (1 << field_width) - 1
            operand_value = (operand_value & field_mask) << int(end)
            
            # Clear the field in the instruction, then set the new value
            instr_mask = ~(field_mask << int(end))
            instr = (instr & instr_mask) | operand_value
        
        logger.debug(f"Built instruction: (hex) {hex(instr)}")
        return instr
                
    def _generate_instruction(self, instr_details: InstructionType, old_instruction: Instruction = None) -> int:
        """
        Generate a new instruction based on the given instruction details and an optional old instruction.
        """
        operands = instr_details.get_var_operands()
        logger.debug(f"Mutating into instruction \"{instr_details.get_mnemonics()}\"")
        return self._build_instruction_with_operands(instr_details, operands, old_instruction)

    def _substitute_opcode(self, instr_details: InstructionType, operands: list[str]) -> int:
        """
        Substitute the opcode of the instruction with the given operands.
        """
        return self._build_instruction_with_operands(instr_details, operands)
        
    def _get_instruction_with_same_operands(self, instr: Instruction) -> InstructionType:
        """
        Get a random InstructionType with the same operands as the given instruction.
        """
        logger.debug(f"Type of instr_type: {type(instr)}")
        instr_type = instr._instr_details
        operands = instr_type.get_var_operands()
        # Get a random instruction type with the same operands
        matching_instructions = [i for i in self._instruction_types if Counter(i.get_var_operands()) == Counter(operands)]
        if not matching_instructions:
            logger.debug(f"No matching instructions found for {instr_type.get_mnemonics()}")
            return instr_type
        return random.choice(matching_instructions)

