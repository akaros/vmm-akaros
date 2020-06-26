import sys

str_struct = "struct"

c_file_path = sys.argv[1]
start_line = int(sys.argv[2]) - 1
end_line = int(sys.argv[3])


def get_word(line, start):
    while start < len(line) and line[start].isspace():
        start += 1
    end = start
    while not line[end].isspace():
        end += 1
    word = line[start:end]
    return word, end


def parse_array_var(field_name):
    end = 0
    while end < len(field_name) and field_name[end] != '[':
        end += 1
    if end == len(field_name):
        return field_name, 1
    else:
        array_size = field_name[end+1: -1]
        member_name = field_name[:end]
        return member_name, array_size


def get_comment(line, start):
    start = 0
    while start < len(line) and not line[start:].startswith("/*"):
        start += 1
    end = start
    while end < len(line) and not line[end:].startswith("*/"):
        end += 1
    return line[start:end+2]


with open(c_file_path) as f:
    lines = f.readlines()
    for line in lines[start_line:end_line]:
        type_name, next_index = get_word(line, 0)
        if type_name == str_struct:
            type_name, next_index = get_word(line, next_index)
        if type_name == "char":
            type_name = "u8"
        # print(type_name, next_index)
        field_name, next_index = get_word(line, next_index)
        if field_name[-1] == ";":
            field_name = field_name[:-1]
        member_name, size = parse_array_var(field_name)
        comment = get_comment(line, next_index)
        if size == 1:
            print("{}:{}, {}".format(member_name, type_name, comment))
        else:
            print("{}:[{};{}], {}".format(
                member_name, type_name, size, comment))
