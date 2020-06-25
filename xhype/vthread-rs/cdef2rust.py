import sys

str_define = "#define"

c_header_file_path = sys.argv[1]
with open(c_header_file_path) as f:
    lines = f.readlines()
    result = []
    for line in lines:
        l = line.strip()
        if l.startswith(str_define):
            start = len(str_define) + 1
            i = start
            while not l[i].isspace():
                i += 1
            def_name = l[start:i]
            def_val = l[i:]
            if "ULL" in l:
                result.append(
                    "pub const {}:u64 = {};".format(def_name, def_val))
            else:
                result.append(
                    "pub const {}:u32 = {};".format(def_name, def_val))

        else:
            result.append(l)
    print("\n".join(result))
