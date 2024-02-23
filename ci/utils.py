import re

def get_project_version_str():
    regex = re.compile(r" +version: '(\d+\.\d+\.\d+)',")
    with open("meson.build") as f:
        for line in f.readlines():
            matches = regex.match(line)
            if matches is not None:
                version_str = matches.group(1)
                return version_str

    raise Exception('meson.build does not have a version string for the project')
