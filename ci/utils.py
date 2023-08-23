import re

def get_first_group(regex, line):
    matches = regex.search(line)
    if matches is None:
        return None
    else:
        return matches.group(1)

def get_configure_ac_version_components():
    major_regex = re.compile(r'^m4_define\(\[rsvg_major_version\],\[(\d+)\]\)')
    minor_regex = re.compile(r'^m4_define\(\[rsvg_minor_version\],\[(\d+)\]\)')
    micro_regex = re.compile(r'^m4_define\(\[rsvg_micro_version\],\[(\d+)\]\)')

    major = None
    micro = None
    minor = None

    with open("configure.ac") as f:
        for line in f.readlines():
            if major is None:
                major = get_first_group(major_regex, line)

            if minor is None:
                minor = get_first_group(minor_regex, line)

            if micro is None:
                micro = get_first_group(micro_regex, line)

    if not (major and minor and micro):
        raise Exception('configure.ac does not have all the necessary version numbers')

    return (major, minor, micro)

def get_configure_ac_version():
    (major, minor, micro) = get_configure_ac_version_components()
    return f'{major}.{minor}.{micro}'
